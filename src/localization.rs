use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language { English, Portuguese }

#[derive(Resource)]
pub struct Localization { pub language: Language }

impl Default for Localization { fn default() -> Self { Self { language: Language::English } } }

impl Localization {
    pub fn text(&self, key: &'static str) -> &'static str {
        match (self.language, key) {
            (Language::Portuguese, "continue") => "Continuar",
            (Language::Portuguese, "new_game") => "Novo jogo",
            (Language::Portuguese, "settings") => "Configurações",
            (Language::Portuguese, "language") => "Idioma",
            (Language::Portuguese, "english") => "Inglês",
            (Language::Portuguese, "portuguese") => "Português",
            (Language::Portuguese, "keep") => "Manter campanha atual",
            (Language::Portuguese, "replace") => "Substituir / Novo jogo",
            (Language::Portuguese, "retry") => "Tentar salvar campanha novamente",
            (Language::Portuguese, "loading_opening") => "Carregando abertura…\nEscape: voltar ao título",
            (Language::Portuguese, "skip") => "Segure Enter para pular",
            (Language::Portuguese, "sleep") => "Dormi assistindo TV de novo…",
            (Language::Portuguese, "work") => "Preciso parar de trabalhar até tão tarde…",
            (Language::Portuguese, "bills") => "Mas o que ganho mal dá para pagar as contas.",
            (Language::Portuguese, "way") => "Vou encontrar um jeito.",
            (Language::Portuguese, "paused") => "PAUSADO",
            (Language::Portuguese, "resume") => "RETOMAR",
            (Language::Portuguese, "settings_upper") => "CONFIGURAÇÕES",
            (Language::Portuguese, "pause_hint") => "Selecione Retomar para continuar. Esc novamente libera o mouse.",
            (Language::Portuguese, "hunger") => "Fome",
            (Language::Portuguese, "boredom") => "Tédio",
            (Language::Portuguese, "stress") => "Estresse",
            (Language::Portuguese, "urge") => "Impulso",
            (Language::Portuguese, "relationships") => "Relacionamentos",
            (Language::Portuguese, "actions_left") => "ações restantes",
            (Language::Portuguese, "morning") => "Manhã",
            (Language::Portuguese, "afternoon") => "Tarde",
            (Language::Portuguese, "evening") => "Noite",
            (Language::Portuguese, "night") => "Madrugada",
            (Language::Portuguese, "day") => "Dia",
            (_, "continue") => "Continue", (_, "new_game") => "New Game", (_, "settings") => "Settings",
            (_, "language") => "Language", (_, "english") => "English", (_, "portuguese") => "Portuguese",
            (_, "keep") => "Keep existing campaign", (_, "replace") => "Replace / New Game",
            (_, "retry") => "Retry campaign save",
            (_, "loading_opening") => "Loading opening…\nEscape: return to title",
            (_, "skip") => "Hold Enter to skip",
            (_, "sleep") => "I've fallen asleep watching TV again...",
            (_, "work") => "I gotta stop working late hours...",
            (_, "bills") => "But what I make is barely enough to cover the bills.",
            (_, "way") => "I'll find a way.",
            (_, "paused") => "PAUSED", (_, "resume") => "RESUME", (_, "settings_upper") => "SETTINGS",
            (_, "pause_hint") => "Select Resume to continue. Esc again releases mouse.",
            (_, "hunger") => "Hunger", (_, "boredom") => "Boredom", (_, "stress") => "Stress",
            (_, "urge") => "Urge", (_, "relationships") => "Relationships",
            (_, "actions_left") => "actions left", (_, "morning") => "Morning",
            (_, "day") => "Day",
            (_, "afternoon") => "Afternoon", (_, "evening") => "Evening", (_, "night") => "Night",
            (_, other) => other,
        }
    }

    pub fn period(&self, label: &str) -> String {
        match label { "Morning" => self.text("morning"), "Afternoon" => self.text("afternoon"), "Evening" => self.text("evening"), "Night" => self.text("night"), _ => label }.into()
    }
}
