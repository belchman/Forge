use flowforge_core::Result;
use flowforge_mcp::McpServer;

pub fn serve() -> Result<()> {
    let server = McpServer::new();
    // MCP server runs on stdio, blocking
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| flowforge_core::Error::Mcp(format!("Failed to create tokio runtime: {e}")))?;

    rt.block_on(async {
        server.run().await
    })
}
