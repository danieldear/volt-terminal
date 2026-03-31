use volt_config::Config;
use volt_ui::App;

fn main() {
    let config = Config::load();
    App::new(config).run();
}
