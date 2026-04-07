mod worker;

fn main() -> anyhow::Result<()> {
    worker::run()
}
