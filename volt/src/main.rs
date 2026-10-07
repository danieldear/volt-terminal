use volt_config::Config;
use volt_ui::App;

mod meow;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--meow") {
        meow::run(&args[1..]);
        return Ok(());
    }
    let (config, config_load_error) = Config::load_with_diagnostics();
    App::new(config, config_load_error)?.run()?;
    Ok(())
}
