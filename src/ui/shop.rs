use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::proj_logic::SpellKind;
use crate::world::entity::ShopOffer;
use crate::world::World;

pub fn draw(frame: &mut Frame, area: Rect, world: &World) {
    let mut lines = vec![
        Line::from(Span::styled(
            "SHOP",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Gold: {}", world.gold)),
        Line::from("Walk totems · Enter buy"),
        Line::from("T target dummy · Tab loadout"),
        Line::from("c / Shift+Enter leave"),
        Line::from(""),
        Line::from(Span::styled(
            "OFFERS",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    if world.shop_dummy_active {
        lines.insert(
            4,
            Line::from(Span::styled(
                "DUMMY ONLINE · autofire",
                Style::default().fg(Color::LightYellow),
            )),
        );
    }

    for (i, t) in world.shop_totems.iter().enumerate() {
        let name = world.shop_offer_label(&t.offer);
        let color = world.shop_offer_color(&t.offer);
        if t.sold {
            lines.push(Line::from(Span::styled(
                format!(" {}. {} — sold", i + 1, name),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }
        let near = world.player.pos.dist(t.pos) < 2.0;
        let prefix = if near { ">" } else { " " };
        let merge = can_merge_tag(world, &t.offer);
        lines.push(Line::from(Span::styled(
            format!("{prefix}{}. {} · {}g{}", i + 1, name, t.price, merge),
            Style::default().fg(color),
        )));
        if let Some(desc) = shop_offer_description(world, &t.offer) {
            lines.push(Line::from(Span::styled(
                format!("    {desc}"),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    lines.push(Line::from(""));
    let near_reroll = world.player.pos.dist(world.shop_reroll_pos) < 2.0;
    lines.push(Line::from(Span::styled(
        format!(
            "{}⟳ Reroll · {}g",
            if near_reroll { ">" } else { " " },
            world.shop_reroll_cost
        ),
        Style::default().fg(if near_reroll {
            Color::LightMagenta
        } else {
            Color::Magenta
        }),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Credits {} · Nucleus {} · Skills {}/{}",
        world.credits,
        world.nucleus.slot_count(),
        world.skills.active.len(),
        world.skills.max_active
    )));

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" After Boss ")),
        area,
    );
}

fn can_merge_tag(world: &World, offer: &ShopOffer) -> String {
    let ShopOffer::Spell(id) = offer else {
        return String::new();
    };
    let Some(def) = world.lib.get(id) else {
        return String::new();
    };
    if !matches!(def.kind, SpellKind::Modifier | SpellKind::Chaos) {
        return String::new();
    }
    if world.count_owned(id) > 0 {
        " [CAN_MERGE]".into()
    } else {
        String::new()
    }
}

fn shop_offer_description(world: &World, offer: &ShopOffer) -> Option<String> {
    match offer {
        ShopOffer::Spell(id) => world.lib.get(id).map(|d| {
            let mut s = d.description.clone();
            if s.chars().count() > 52 {
                s = s.chars().take(49).collect::<String>() + "...";
            }
            s
        }),
        ShopOffer::Skill(id) => {
            let d = crate::skills::def(*id);
            let mut s = d.description.to_string();
            if s.chars().count() > 52 {
                s = s.chars().take(49).collect::<String>() + "...";
            }
            Some(s)
        }
        ShopOffer::Credit => Some("Gain a re-animation credit.".into()),
        ShopOffer::Heal => Some("Restore 35 HP.".into()),
        ShopOffer::SkillSlot => Some("Unlock one more skill slot.".into()),
    }
}
