use ratatui::style::Color;
use tachyonfx::{Duration, EffectManager, fx, Motion};

/// Room-transition polish only. Combat juice lives as local arena/HUD VFX.
pub struct FxBus {
    effects: EffectManager<()>,
    pending_dt_ms: u32,
}

impl FxBus {
    pub fn new() -> Self {
        Self {
            effects: EffectManager::default(),
            pending_dt_ms: 0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.pending_dt_ms = (dt * 1000.0).max(0.0) as u32;
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        let ms = self.pending_dt_ms;
        self.pending_dt_ms = 0;
        let area = frame.area();
        self.effects
            .process_effects(Duration::from_millis(ms), frame.buffer_mut(), area);
    }

    pub fn on_room_enter(&mut self) {
        // Swipe left: new room slides in from the right.
        let effect = fx::slide_in(
            Motion::RightToLeft,
            12,
            0,
            Color::Black,
            (420, tachyonfx::Interpolation::QuadOut),
        );
        self.effects.add_effect(effect);
    }

    pub fn on_boss(&mut self) {
        let effect = fx::coalesce(500);
        self.effects.add_effect(effect);
    }

    pub fn on_shop(&mut self) {
        let effect = fx::sweep_in(
            Motion::UpToDown,
            8,
            0,
            Color::Rgb(20, 20, 10),
            (400, tachyonfx::Interpolation::SineOut),
        );
        self.effects.add_effect(effect);
    }

    pub fn on_death(&mut self) {
        let effect = fx::dissolve(900);
        self.effects.add_effect(effect);
    }
}
