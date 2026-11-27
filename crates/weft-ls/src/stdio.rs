//! 标准 LSP 3.17 stdio 壳层（供 `weft lsp` 调用），复用传输无关的 `LspServer`。

use lsp_server::Connection;

use crate::server::LspServer;

pub fn run_stdio(default_lang: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (connection, io_threads) = Connection::stdio();
    let mut server = LspServer::new(default_lang);

    for msg in &connection.receiver {
        let (outgoing, should_exit) = server.handle_message(msg);
        for reply in outgoing {
            connection.sender.send(reply)?;
        }
        if should_exit {
            break;
        }
    }

    drop(connection);
    io_threads.join()?;
    Ok(())
}
