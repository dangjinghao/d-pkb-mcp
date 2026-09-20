mod server;
mod tools;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run().await
}
