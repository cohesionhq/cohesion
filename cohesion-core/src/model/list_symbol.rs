use cohesion_macros::{TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// The symbol a task folder or a saved task view is drawn with, from a fixed
/// set of everyday things. Each client maps a name to an icon of its own (the
/// web UI to a Lucide icon, the iPhone app to an SF Symbol), so only the name
/// travels; "List symbols" in `docs/architecture.md` lists both.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ListSymbol {
    Home,
    Work,
    Inbox,
    Star,
    Heart,
    Flag,
    Bookmark,
    Book,
    Cart,
    Money,
    Calendar,
    People,
    Person,
    Health,
    Fitness,
    Travel,
    Car,
    Food,
    Code,
    Idea,
    Music,
    Camera,
    Gift,
    School,
    Tools,
    Garden,
    Pets,
    Bug,
    Globe,
    Chat,
    Mail,
    Lock,
}
