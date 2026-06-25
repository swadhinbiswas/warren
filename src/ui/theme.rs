use console::Style;

pub struct Theme {
    pub brand: Style,
    pub success: Style,
    pub error: Style,
    pub warning: Style,
    pub info: Style,
    pub dim: Style,
    pub bold: Style,
    pub label: Style,
    pub value: Style,
}

impl Theme {
    pub fn new() -> Self {
        Self {
            brand: Style::new().cyan().bold(),
            success: Style::new().green().bold(),
            error: Style::new().red().bold(),
            warning: Style::new().yellow().bold(),
            info: Style::new().blue(),
            dim: Style::new().dim(),
            bold: Style::new().bold(),
            label: Style::new().white().dim(),
            value: Style::new().white().bold(),
        }
    }

    pub fn prefix(&self) -> String {
        format!("  {}  ", self.brand.apply_to("warren"))
    }

    pub fn success(&self, msg: &str) {
        eprintln!("  {}  {}", self.success.apply_to("✓"), msg);
    }

    pub fn error(&self, msg: &str) {
        eprintln!("  {}  {}", self.error.apply_to("✗"), msg);
    }

    pub fn warn(&self, msg: &str) {
        eprintln!("  {}  {}", self.warning.apply_to("!"), msg);
    }

    pub fn step(&self, icon: &str, msg: &str) {
        eprintln!("  {}  {}", self.info.apply_to(icon), msg);
    }

    pub fn header(&self, msg: &str) {
        eprintln!("\n  {}  {}\n", self.brand.apply_to("warren"), self.bold.apply_to(msg));
    }

    pub fn kv(&self, key: &str, value: &str) {
        eprintln!("  {:<12}{}", self.label.apply_to(key), self.value.apply_to(value));
    }

    pub fn dim(&self, msg: &str) {
        eprintln!("  {}", self.dim.apply_to(msg));
    }

    pub fn blank(&self) {
        eprintln!();
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}
