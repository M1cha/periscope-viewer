use eframe::egui;
use serde::Deserialize as _;
use std::io::BufRead as _;
use std::io::Write as _;

macro_rules! enum_with_fromstr {
    ( $name:ident, $($ident:ident),+) => {
        #[derive(Debug, Copy, Clone, PartialEq, serde::Deserialize)]
        enum $name {
            $($ident,)+
        }

        impl TryFrom<&str> for $name {
            type Error = &'static str;

            fn try_from(s: &str) -> Result<$name, &'static str> {
                match s {
                    $(stringify!($ident) => Ok($name::$ident),)+
                    _ => Err("Invalid String")
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
pub struct StickState {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
struct ControllerState {
    id: u8,
    c: u8,
    bs: u32,
    ls: StickState,
    rs: StickState,
}

type SharedControllerStates = std::sync::Arc<std::sync::RwLock<Vec<ControllerState>>>;

#[derive(Debug, serde::Deserialize)]
struct ControllerConfig {
    id: u8,
    layout: String,
    position: egui::Vec2,
}

enum_with_fromstr! {
ConditionValue,
    ButtonA,
    ButtonB,
    ButtonX,
    ButtonY,
    ButtonStickLeft,
    ButtonStickRight,
    ButtonL,
    ButtonR,
    ButtonZL,
    ButtonZR,
    ButtonPlus,
    ButtonMinus,
    ButtonDpadLeft,
    ButtonDpadUp,
    ButtonDpadRight,
    ButtonDpadDown,
    ButtonCapture,
    ButtonHome,
    StickLeftActive,
    StickRightActive,
    Connected,
    Connected0,
    Connected1,
    Connected2,
    Connected3,
    Connected4,
    Connected5,
    Connected6,
    Connected7
}

#[derive(Debug, serde::Deserialize)]
struct Condition {
    not: bool,
    value: ConditionValue,
}

struct ColorFromString;

impl<'de> serde_with::DeserializeAs<'de, egui::Color32> for ColorFromString {
    fn deserialize_as<D>(deserializer: D) -> Result<egui::Color32, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer).map_err(serde::de::Error::custom)?;
        let mut rgba = [0u8; 4];

        for (i, item) in rgba.iter_mut().enumerate() {
            let byte_str = s
                .get(i * 2..i * 2 + 2)
                .ok_or_else(|| serde::de::Error::custom("wrong length"))?;
            let byte = u8::from_str_radix(byte_str, 16).map_err(serde::de::Error::custom)?;
            *item = byte;
        }

        Ok(egui::Color32::from_rgba_unmultiplied(
            rgba[0], rgba[1], rgba[2], rgba[3],
        ))
    }
}

struct StrokeFromStrokeConfig;

impl<'de> serde_with::DeserializeAs<'de, egui::Stroke> for StrokeFromStrokeConfig {
    fn deserialize_as<D>(deserializer: D) -> Result<egui::Stroke, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let config = StrokeConfig::deserialize(deserializer).map_err(serde::de::Error::custom)?;

        Ok(egui::Stroke {
            width: config.width,
            color: config.color,
        })
    }
}

#[serde_with::serde_as]
#[derive(Debug, serde::Deserialize)]
pub struct StrokeConfig {
    pub width: f32,
    #[serde_as(as = "ColorFromString")]
    pub color: egui::Color32,
}

#[serde_with::serde_as]
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ItemTypeData {
    Image {
        path: String,
    },
    Text {
        value: String,
        #[serde_as(as = "ColorFromString")]
        color: egui::Color32,
        size: f32,
    },
    Rectangle {
        size: egui::Vec2,
        #[serde_as(as = "ColorFromString")]
        fill_color: egui::Color32,
        #[serde_as(as = "StrokeFromStrokeConfig")]
        #[serde(default)]
        stroke: egui::Stroke,
    },
    Circle {
        radius: f32,
        #[serde_as(as = "ColorFromString")]
        fill_color: egui::Color32,
        #[serde_as(as = "StrokeFromStrokeConfig")]
        #[serde(default)]
        stroke: egui::Stroke,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum PositionModifier {
    StickLeft { range: f32 },
    StickRight { range: f32 },
}

struct ConditionFromString;

impl<'de> serde_with::DeserializeAs<'de, Condition> for ConditionFromString {
    fn deserialize_as<D>(deserializer: D) -> Result<Condition, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer).map_err(serde::de::Error::custom)?;
        let (not, string) = match string.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, string.as_str()),
        };
        let value = ConditionValue::try_from(string).map_err(serde::de::Error::custom)?;

        Ok(Condition { not, value })
    }
}

#[serde_with::serde_as]
#[derive(Debug, serde::Deserialize)]
struct Item {
    #[serde(flatten)]
    r#type: ItemTypeData,
    position: egui::Pos2,
    #[serde(default)]
    position_modifier: Option<PositionModifier>,
    #[serde_as(as = "Vec<ConditionFromString>")]
    #[serde(default)]
    #[serde(rename = "if")]
    condition: Vec<Condition>,
}

impl Item {
    pub fn render(
        &self,
        config: &Config,
        painter: &egui::Painter,
        ui: &egui::Ui,
        item_position: egui::Pos2,
    ) {
        match &self.r#type {
            ItemTypeData::Image { path } => {
                let image = egui::Image::new(format!("file://{path}"));
                if let Some(size) = image.load_and_calc_size(ui, egui::Vec2::INFINITY) {
                    image.paint_at(
                        ui,
                        egui::Rect::from_min_size(
                            item_position,
                            egui::vec2(size.x * config.scale, size.y * config.scale),
                        ),
                    );
                }
            }
            ItemTypeData::Circle {
                radius,
                fill_color,
                stroke,
            } => {
                let center = egui::pos2(
                    item_position.x + radius * config.scale,
                    item_position.y + radius * config.scale,
                );
                let radius = radius * config.scale;

                painter.circle(center, radius, *fill_color, *stroke);
            }
            ItemTypeData::Rectangle {
                size,
                fill_color,
                stroke,
            } => {
                let rect = egui::Rect::from_min_size(item_position, *size * config.scale);

                painter.rect(rect, egui::Rounding::ZERO, *fill_color, *stroke);
            }
            ItemTypeData::Text { value, color, size } => {
                painter.text(
                    item_position,
                    egui::Align2::CENTER_CENTER,
                    value,
                    egui::FontId::proportional(size * config.scale),
                    *color,
                );
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct Layout {
    name: String,
    #[serde(default)]
    items: Vec<Item>,
}

#[derive(Debug, serde::Deserialize)]
struct Config {
    scale: f32,
    size: egui::Vec2,
    controllers: Vec<ControllerConfig>,
    #[serde(default)]
    layouts: Vec<Layout>,
    #[serde(default)]
    items: Vec<Item>,
}

fn load_config(path: String) -> Config {
    let contents = std::fs::read_to_string(path).expect("Failed to read config");
    toml::from_str(&contents).expect("Failed to parse config")
}

fn spawn_client(addr: String, shared_controller_states: SharedControllerStates) {
    let mut stream =
        std::net::TcpStream::connect(format!("{addr}:2579")).expect("Failed to connect");
    std::thread::spawn(move || {
        let mut message = Vec::with_capacity(810);
        let mut reader =
            std::io::BufReader::new(stream.try_clone().expect("Failed to clone tcp stream"));
        loop {
            stream
                .write_all(&[b'1'])
                .expect("Failed to write to tcp stream");

            message.clear();
            let num_read = reader
                .read_until(b']', &mut message)
                .expect("Failed to read");
            let message = &message[..num_read];
            let controller_states: Vec<ControllerState> = match serde_json::from_slice(message) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Failed to decode: {e}");
                    continue;
                }
            };
            //eprintln!("{controller_states:#?}");

            *shared_controller_states.write().unwrap() = controller_states;
        }
    });
}

struct App {
    config: Config,
    shared_controller_states: SharedControllerStates,
}

static BUTTON_CONDITIONS: &[ConditionValue] = &[
    ConditionValue::ButtonA,
    ConditionValue::ButtonB,
    ConditionValue::ButtonX,
    ConditionValue::ButtonY,
    ConditionValue::ButtonStickLeft,
    ConditionValue::ButtonStickRight,
    ConditionValue::ButtonL,
    ConditionValue::ButtonR,
    ConditionValue::ButtonZL,
    ConditionValue::ButtonZR,
    ConditionValue::ButtonPlus,
    ConditionValue::ButtonMinus,
    ConditionValue::ButtonDpadLeft,
    ConditionValue::ButtonDpadUp,
    ConditionValue::ButtonDpadRight,
    ConditionValue::ButtonDpadDown,
];

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::BLACK.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let controller_states = self.shared_controller_states.read().unwrap();
            let config = &self.config;
            let painter = ui.painter();
            let mut active_conditions = Vec::with_capacity(20);

            active_conditions.clear();
            for controller_state in controller_states.iter() {
                if controller_state.c != 1 {
                    continue;
                }

                match controller_state.id {
                    0 => active_conditions.push(ConditionValue::Connected0),
                    1 => active_conditions.push(ConditionValue::Connected1),
                    2 => active_conditions.push(ConditionValue::Connected2),
                    3 => active_conditions.push(ConditionValue::Connected3),
                    4 => active_conditions.push(ConditionValue::Connected4),
                    5 => active_conditions.push(ConditionValue::Connected5),
                    6 => active_conditions.push(ConditionValue::Connected6),
                    7 => active_conditions.push(ConditionValue::Connected7),
                    _ => (),
                }
            }

            'item_loop: for item in &config.items {
                for condition in &item.condition {
                    let found = active_conditions.iter().any(|c| c == &condition.value);
                    if found != !condition.not {
                        continue 'item_loop;
                    }
                }
                let item_position = egui::pos2(
                    item.position.x * config.scale,
                    item.position.y * config.scale,
                );
                item.render(config, painter, ui, item_position);
            }

            for controller in &config.controllers {
                active_conditions.clear();
                let layout = config
                    .layouts
                    .iter()
                    .find(|l| l.name == controller.layout)
                    .expect("unknown layout");
                let controller_state =
                    match controller_states.iter().find(|s| s.id == controller.id) {
                        Some(v) => v,
                        None => continue,
                    };

                if controller_state.c == 1 {
                    active_conditions.push(ConditionValue::Connected);
                }

                for (bit, condition) in BUTTON_CONDITIONS.iter().enumerate().take(16) {
                    if controller_state.bs & (1 << bit) != 0 {
                        active_conditions.push(*condition);
                    }
                }

                'item_loop: for item in &layout.items {
                    for condition in &item.condition {
                        let found = active_conditions.iter().any(|c| c == &condition.value);
                        if found != !condition.not {
                            continue 'item_loop;
                        }
                    }

                    let item_position = egui::pos2(
                        (controller.position.x + item.position.x) * config.scale,
                        (controller.position.y + item.position.y) * config.scale,
                    );
                    let item_position = match item.position_modifier {
                        Some(PositionModifier::StickLeft { range }) => egui::pos2(
                            item_position.x
                                + (range * config.scale) / 32767.0 * controller_state.ls.x,
                            item_position.y
                                - (range * config.scale) / 32767.0 * controller_state.ls.y,
                        ),
                        Some(PositionModifier::StickRight { range }) => egui::pos2(
                            item_position.x
                                + (range * config.scale) / 32767.0 * controller_state.rs.x,
                            item_position.y
                                - (range * config.scale) / 32767.0 * controller_state.rs.y,
                        ),
                        None => item_position,
                    };

                    item.render(config, painter, ui, item_position);
                }
            }
        });

        ctx.request_repaint();
    }
}

fn main() {
    let addr = std::env::args().nth(1).expect("Missing address argument");
    let config_path = std::env::args().nth(2).expect("Missing config argument");
    let config = load_config(config_path);
    eprintln!("{config:#?}");

    let shared_controller_states = SharedControllerStates::default();
    spawn_client(addr, shared_controller_states.clone());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.size.x * config.scale, config.size.y * config.scale])
            .with_decorations(false)
            .with_resizable(false)
            .with_maximized(false)
            .with_fullscreen(false),
        ..Default::default()
    };
    eframe::run_native(
        "Periscope",
        options,
        Box::new(|cc| {
            cc.egui_ctx.style_mut(|style| {
                style.visuals.panel_fill = egui::Color32::TRANSPARENT;
                style.visuals.window_fill = egui::Color32::TRANSPARENT;
            });

            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "default".to_owned(),
                egui::FontData::from_static(include_bytes!(
                    "/usr/share/fonts/TTF/OpenSans-ExtraBold.ttf"
                )),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "default".to_owned());
            cc.egui_ctx.set_fonts(fonts);

            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(App {
                config,
                shared_controller_states,
            })
        }),
    )
    .expect("egui failed");
}
