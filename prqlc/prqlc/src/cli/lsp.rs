use std::error::Error;

use lsp_types::OneOf;
use lsp_types::{request::GotoDefinition, GotoDefinitionResponse, ServerCapabilities};

use lsp_server::{Connection, ErrorCode, ExtractError, Message, Request, RequestId, Response};

pub fn run() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("starting PRQL LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })
    .expect("`ServerCapabilities` is always serializable");
    // The client's initialization params aren't used yet, so we deliberately
    // don't deserialize them: a client that sends a payload our `lsp-types`
    // version doesn't model would otherwise take the server down before the
    // main loop even starts.
    if let Err(e) = connection.initialize(server_capabilities) {
        if e.channel_is_disconnected() {
            io_threads.join()?;
        }
        return Err(e.into());
    }
    main_loop(connection)?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("shutting down server");
    Ok(())
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    eprintln!("starting main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {msg:?}");
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("got request: {req:?}");
                let id = req.id.clone();
                match cast::<GotoDefinition>(req) {
                    Ok((id, params)) => {
                        eprintln!("got gotoDefinition request #{id}: {params:?}");
                        let result = Some(GotoDefinitionResponse::Array(Vec::new()));
                        let resp = Response::new_ok(id, result);
                        connection.sender.send(Message::Response(resp))?;
                        continue;
                    }
                    // Params that don't deserialize are a client-side problem —
                    // often just a version skew against our `lsp-types`. Reply
                    // with an error for that one request rather than taking the
                    // server down.
                    Err(ExtractError::JsonError { method, error }) => {
                        let resp = Response::new_err(
                            id,
                            ErrorCode::InvalidParams as i32,
                            format!("invalid params for `{method}`: {error}"),
                        );
                        connection.sender.send(Message::Response(resp))?;
                        continue;
                    }
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                // ...
            }
            Message::Response(resp) => {
                eprintln!("got response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("got notification: {not:?}");
                // Only the `exit` notification should stop the loop. Other
                // notifications (e.g. `textDocument/didOpen`, `$/setTrace`)
                // are logged and ignored — returning on any notification
                // would shut the server down as soon as a client sends one.
                if not.method == "exit" {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

#[cfg(test)]
mod tests {
    use super::main_loop;
    use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId};

    /// A `textDocument/definition` request whose params fail to deserialize
    /// used to `panic!`, taking the whole server down. It should get an error
    /// response and the loop should keep running.
    #[test]
    fn invalid_params_get_an_error_response() {
        let (server, client) = Connection::memory();
        let server_thread = std::thread::spawn(move || main_loop(server));

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "textDocument/definition".to_string(),
                params: serde_json::json!({ "not": "a position" }),
            }))
            .unwrap();

        let Message::Response(resp) = client.receiver.recv().unwrap() else {
            panic!("expected a response");
        };
        assert_eq!(resp.id, RequestId::from(1));
        let err = resp.response_result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidParams as i32);

        // The loop survived the bad request and still handles later messages.
        client
            .sender
            .send(Message::Notification(Notification {
                method: "exit".to_string(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        server_thread.join().unwrap().unwrap();
    }
}
