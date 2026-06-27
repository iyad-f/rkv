// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Response;
use crate::server::State;

/// `AUTH [username] password` authenticates the connection.
///
/// The lone-password form authenticates as the default user. A username may be
/// given for the two-argument form, but only `default` is recognized.
pub const AUTH: Command = Command {
    name: "AUTH",
    arity: Arity::Min(2),
    write: false,
    auth_required: false,
    handler: auth,
};

fn auth(ctx: &mut Context, state: &mut State) -> Response {
    let (username, password) = match ctx.args {
        [password] => (None, password),
        [username, password] => (Some(username), password),
        _ => return errors::syntax_error(),
    };

    // Only the default user exists, so any other username fails. The lone
    // password form targets the default user.
    let is_default = username.is_none_or(|name| name.as_slice() == b"default");

    match &state.config.password {
        // With no password set the default user accepts any password, but the
        // lone-password form still reports the misconfiguration.
        None if username.is_none() => Response::Error(
            "ERR AUTH <password> called without any password configured for the default user. \
             Are you sure your configuration is correct?"
                .to_string(),
        ),
        None if is_default => {
            ctx.session.authenticate();
            Response::Simple("OK".to_string())
        }
        Some(required) if is_default && password == required.as_bytes() => {
            ctx.session.authenticate();
            Response::Simple("OK".to_string())
        }
        _ => Response::Error(
            "WRONGPASS invalid username-password pair or user is disabled.".to_string(),
        ),
    }
}

/// `ECHO message` replies with the message.
pub const ECHO: Command = Command {
    name: "ECHO",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: echo,
};

fn echo(ctx: &mut Context, _state: &mut State) -> Response {
    let [message] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    Response::Bulk(message.clone())
}

/// `PING [message]` replies with PONG, or echoes the optional message.
pub const PING: Command = Command {
    name: "PING",
    arity: Arity::Min(1),
    write: false,
    auth_required: true,
    handler: ping,
};

fn ping(ctx: &mut Context, _state: &mut State) -> Response {
    match ctx.args {
        [] => Response::Simple("PONG".to_string()),
        [message] => Response::Bulk(message.clone()),
        _ => errors::wrong_args(ctx.command.name),
    }
}

/// `QUIT` closes the connection.
pub const QUIT: Command = Command {
    name: "QUIT",
    arity: Arity::Min(1),
    write: false,
    auth_required: false,
    handler: quit,
};

fn quit(ctx: &mut Context, _state: &mut State) -> Response {
    ctx.session.request_close();
    Response::Simple("OK".to_string())
}

/// `RESET` resets the session to its initial state.
pub const RESET: Command = Command {
    name: "RESET",
    arity: Arity::Exact(1),
    write: false,
    auth_required: false,
    handler: reset,
};

fn reset(ctx: &mut Context, _state: &mut State) -> Response {
    ctx.session.reset();
    Response::Simple("RESET".to_string())
}

#[cfg(test)]
mod tests {
    use crate::command::test_utils::{cmd, dispatch, state};
    use crate::resp::Response;
    use crate::session::Session;

    /// State that requires the password `secret`.
    fn protected_state() -> crate::server::State {
        let mut state = state();
        state.config.password = Some("secret".to_string());
        state
    }

    // AUTH

    #[test]
    fn auth_with_no_password_set_reports_misconfiguration() {
        assert_eq!(
            dispatch(&cmd(&["AUTH", "foo"]), &mut state()),
            Response::Error(
                "ERR AUTH <password> called without any password configured for the default \
                 user. Are you sure your configuration is correct?"
                    .to_string()
            )
        );
    }

    #[test]
    fn auth_default_user_with_no_password_set_succeeds() {
        let mut session = Session::default();
        let reply = crate::command::dispatch(
            &cmd(&["AUTH", "default", "foo"]),
            &mut state(),
            &mut session,
        );

        assert_eq!(reply, Response::Simple("OK".to_string()));
        assert!(session.is_authenticated());
    }

    #[test]
    fn auth_unknown_user_fails() {
        assert_eq!(
            dispatch(&cmd(&["AUTH", "bob", "foo"]), &mut state()),
            Response::Error(
                "WRONGPASS invalid username-password pair or user is disabled.".to_string()
            )
        );
    }

    #[test]
    fn auth_too_many_args_is_syntax_error() {
        assert_eq!(
            dispatch(&cmd(&["AUTH", "a", "b", "c"]), &mut state()),
            Response::Error("ERR syntax error".to_string())
        );
    }

    #[test]
    fn auth_correct_password_authenticates() {
        let mut session = Session::default();
        let reply = crate::command::dispatch(
            &cmd(&["AUTH", "secret"]),
            &mut protected_state(),
            &mut session,
        );

        assert_eq!(reply, Response::Simple("OK".to_string()));
        assert!(session.is_authenticated());
    }

    #[test]
    fn auth_wrong_password_fails() {
        let mut session = Session::default();
        let reply = crate::command::dispatch(
            &cmd(&["AUTH", "wrong"]),
            &mut protected_state(),
            &mut session,
        );

        assert_eq!(
            reply,
            Response::Error(
                "WRONGPASS invalid username-password pair or user is disabled.".to_string()
            )
        );
        assert!(!session.is_authenticated());
    }

    #[test]
    fn command_rejected_before_auth_then_allowed_after() {
        let mut state = protected_state();
        let mut session = Session::default();

        assert_eq!(
            crate::command::dispatch(&cmd(&["PING"]), &mut state, &mut session),
            Response::Error("NOAUTH Authentication required.".to_string())
        );

        crate::command::dispatch(&cmd(&["AUTH", "secret"]), &mut state, &mut session);

        assert_eq!(
            crate::command::dispatch(&cmd(&["PING"]), &mut state, &mut session),
            Response::Simple("PONG".to_string())
        );
    }

    // ECHO

    #[test]
    fn returns_argument() {
        assert_eq!(
            dispatch(&cmd(&["ECHO", "hello"]), &mut state()),
            Response::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn echo_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["ECHO"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'echo' command".to_string())
        );
    }

    // PING

    #[test]
    fn no_arg_replies_pong() {
        assert_eq!(
            dispatch(&cmd(&["PING"]), &mut state()),
            Response::Simple("PONG".to_string())
        );
    }

    #[test]
    fn echoes_message() {
        assert_eq!(
            dispatch(&cmd(&["PING", "hi"]), &mut state()),
            Response::Bulk(b"hi".to_vec())
        );
    }

    #[test]
    fn too_many_args() {
        assert_eq!(
            dispatch(&cmd(&["PING", "a", "b"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'ping' command".to_string())
        );
    }

    // QUIT

    #[test]
    fn quit_replies_ok_and_requests_close() {
        let mut session = Session::default();
        let reply = crate::command::dispatch(&cmd(&["QUIT"]), &mut state(), &mut session);

        assert_eq!(reply, Response::Simple("OK".to_string()));
        assert!(session.should_close());
    }

    #[test]
    fn quit_ignores_extra_args() {
        let mut session = Session::default();
        let reply = crate::command::dispatch(&cmd(&["QUIT", "x", "y"]), &mut state(), &mut session);

        assert_eq!(reply, Response::Simple("OK".to_string()));
        assert!(session.should_close());
    }

    // RESET

    #[test]
    fn reset_replies_reset() {
        assert_eq!(
            dispatch(&cmd(&["RESET"]), &mut state()),
            Response::Simple("RESET".to_string())
        );
    }

    #[test]
    fn reset_deauthenticates() {
        let mut state = protected_state();
        let mut session = Session::default();
        crate::command::dispatch(&cmd(&["AUTH", "secret"]), &mut state, &mut session);
        assert!(session.is_authenticated());

        let reply = crate::command::dispatch(&cmd(&["RESET"]), &mut state, &mut session);

        assert_eq!(reply, Response::Simple("RESET".to_string()));
        assert!(!session.is_authenticated());
    }

    #[test]
    fn reset_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["RESET", "x"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'reset' command".to_string())
        );
    }
}
