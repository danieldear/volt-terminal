use volt_config::Config;
use volt_ui::App;

fn main() -> anyhow::Result<()> {
    let (config, config_load_error) = Config::load_with_diagnostics();
    App::new(config, config_load_error)?.run()?;
    Ok(())
}
