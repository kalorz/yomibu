pub mod credentials;
pub mod options;

use options::Setting;

#[derive(Debug, Clone, Copy)]
pub struct Component {
    pub id: &'static str,
    pub settings: &'static [Setting],
}
impl Component {
    pub fn setting(self, name: &str) -> Option<Setting> {
        self.settings
            .iter()
            .copied()
            .find(|setting| setting.name().option == name)
    }
}
