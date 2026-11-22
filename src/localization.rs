use crate::game::campaign::{BillKind, HouseholdItem};
use crate::preferences;
use bevy::prelude::*;

// Read by web/index.html before the game starts.
const STORAGE_KEY: &str = "casino-game.language.v1";

macro_rules! format_text {
    ($localization:expr, $english:literal, $portuguese:literal $(, $argument:expr)* $(,)?) => {
        match $localization.language {
            $crate::localization::Language::English => format!($english $(, $argument)*),
            $crate::localization::Language::Portuguese => format!($portuguese $(, $argument)*),
        }
    };
}
pub(crate) use format_text;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    English,
    Portuguese,
}

impl Language {
    fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Portuguese => "pt-BR",
        }
    }
}

#[derive(Event)]
pub struct LanguageSelected;

#[derive(Resource)]
pub struct Localization {
    pub language: Language,
}

impl Default for Localization {
    fn default() -> Self {
        Self {
            language: Language::Portuguese,
        }
    }
}

impl Localization {
    pub fn load() -> Self {
        let saved = preferences::read(STORAGE_KEY).ok().flatten();
        Self::from_saved_language(saved.as_deref())
    }

    fn from_saved_language(saved: Option<&str>) -> Self {
        let tag = saved.and_then(|json| serde_json::from_str::<String>(json).ok());
        if tag.as_deref() == Some(Language::English.tag()) {
            return Self {
                language: Language::English,
            };
        }
        Self::default()
    }

    pub fn save_selection(_: On<LanguageSelected>, localization: Res<Self>) {
        let tag = localization.language.tag();
        let result = serde_json::to_string(tag)
            .map_err(|error| error.to_string())
            .and_then(|json| preferences::write(STORAGE_KEY, &json));
        if let Err(error) = result {
            warn!("Cannot save language preference: {error}");
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let _ = root.set_attribute("lang", tag);
        }
    }

    #[cfg(test)]
    pub const ENGLISH: Self = Self {
        language: Language::English,
    };

    pub fn choose<'a>(&self, english: &'a str, portuguese: &'a str) -> &'a str {
        match self.language {
            Language::English => english,
            Language::Portuguese => portuguese,
        }
    }

    pub fn reais(&self, cents: impl Into<i128>) -> String {
        let cents = cents.into();
        let amount = cents.unsigned_abs();
        let sign = if cents < 0 { "-" } else { "" };
        let decimal = self.choose(".", ",");
        format!("{sign}R${}{decimal}{:02}", amount / 100, amount % 100)
    }

    pub fn bill(&self, kind: BillKind) -> &'static str {
        self.text(match kind {
            BillKind::Water => "bill_water",
            BillKind::Electricity => "bill_electricity",
            BillKind::Rent => "bill_rent",
        })
    }

    pub fn item(&self, item: HouseholdItem) -> &'static str {
        self.choose(
            item.label(),
            match item {
                HouseholdItem::Tv => "TV",
                HouseholdItem::Fridge => "Geladeira",
                HouseholdItem::Stove => "Fogão",
                HouseholdItem::Cookware => "Utensílios de cozinha",
                HouseholdItem::Couch => "Sofá",
                HouseholdItem::Books => "Livros",
                HouseholdItem::Pipoca => "Pipoca",
            },
        )
    }

    pub fn sale_loss(&self, item: HouseholdItem) -> &'static str {
        self.choose(item.sale_loss(), match item {
            HouseholdItem::Tv => "A TV sai de casa definitivamente. Você perde a opção de assistir à TV.",
            HouseholdItem::Fridge => "A geladeira sai de casa definitivamente. Você perde as porções guardadas. Ainda dá para pedir comida pronta.",
            HouseholdItem::Stove | HouseholdItem::Cookware => "Este item sai de casa definitivamente. Você fica sem poder cozinhar ou guardar mantimentos, mas ainda pode pedir comida pronta.",
            HouseholdItem::Couch => "O sofá sai de casa definitivamente.",
            HouseholdItem::Books => "Os seis livros saem de casa definitivamente. A estante fica.",
            HouseholdItem::Pipoca => "O Pipoca sai de casa definitivamente. Isso ENCERRA A CAMPANHA imediatamente.",
        })
    }

    pub fn action_error(&self, error: &str) -> String {
        if self.language == Language::English {
            return error.into();
        }
        if let Some(cost) = error
            .strip_prefix("This action needs ")
            .and_then(|rest| rest.split_once(" PA."))
            .and_then(|(cost, _)| cost.parse::<u8>().ok())
        {
            return format!(
                "Essa ação custa {cost} PA. Durma para começar o dia seguinte. Ações gratuitas continuam disponíveis."
            );
        }
        match error {
            "Too hungry. Eat something first." => self.text("too_hungry"),
            "I'm too stressed. I need to calm down first." => "Estou estressado demais. Preciso me acalmar primeiro.",
            "Work shifts must start before night." => self.text("work_at_night"),
            "Sleep to open the daily report first." => "Durma para abrir o resumo do dia.",
            "Continue from the daily report before taking another action." => "Avance pelo resumo do dia antes de realizar outra ação.",
            "This campaign has ended. History and reports remain available." => "A campanha terminou. Você ainda pode consultar o histórico e os resumos.",
            "MeFood has no shift today." => "Hoje não há turno de entregas pelo MeFood.",
            "Today's MeFood shift is already complete." => "Você já fez as entregas de hoje pelo MeFood.",
            "There is no more room for groceries." => "Não há espaço para mais mantimentos.",
            "No food portions left. Buy groceries or order prepared food." => "Acabaram as porções. Compre mantimentos ou peça comida pronta.",
            "The TV has been sold. Try another activity." => "A TV foi vendida. Tente outra atividade.",
            "The power is off. Pay the electricity bill to use the TV." => "A energia foi cortada. Pague a conta de luz para usar a TV.",
            "Pipoca is no longer at home." => "O Pipoca não está mais no apartamento.",
            "Betting is already blocked until tomorrow morning." => "As apostas já estão bloqueadas até amanhã de manhã.",
            "Cooking and grocery storage need the fridge, stove and cookware. Order prepared food instead." => "Você precisa de geladeira, fogão e utensílios para cozinhar e guardar mantimentos. Peça comida pronta.",
            "The daily report is missing; the next day cannot start." => "O resumo do dia está ausente. Não foi possível começar o próximo dia.",
            "Insufficient balance or amount is too large." => "Saldo insuficiente ou valor acima do permitido.",
            "Daily financial totals exceed the supported range. Reduce the stake or finish the day before betting again." => "O total de despesas e receitas do dia passou do limite. Reduza o valor da aposta ou encerre o dia antes de apostar de novo.",
            "The stake is too large." => "O valor da aposta é alto demais.",
            "Enter a whole-real stake of at least R$1." => "Digite um valor inteiro em reais, de pelo menos R$1.",
            "The stake exceeds your available balance. Lower the stake." => "O valor da aposta é maior que seu saldo disponível. Aposte um valor menor.",
            "Betting is blocked until tomorrow morning. Support stays available." => "As apostas estão bloqueadas até amanhã de manhã. O app de Apoio continua disponível.",
            "This stake exceeds your remaining daily wagering cap." => "O valor da aposta ultrapassa o restante do seu limite diário.",
            "No more betting sessions are available." => "Não há mais sessões de apostas disponíveis.",
            "End this session before another round." => "Encerre a sessão atual antes de iniciar outra rodada.",
            "This bill is unavailable in the campaign." => "Esta conta não está disponível na campanha.",
            "This bill is already paid. See History for the payment." => "Esta conta já foi paga. Consulte o pagamento no Histórico.",
            "Rent is due by the end of the last day." => "O aluguel vence no fim do último dia.",
            "The one-time rent extension is already in place: due end of day 10." => "O aluguel já foi prorrogado. Agora vence no fim do dia 10.",
            "Rent is already paid; no extension is needed." => "O aluguel já foi pago.",
            "Rent must be negotiated before housing loss at the start of day 9." => "Negocie o aluguel antes do início do dia 9 para evitar o despejo.",
            "No scheduled visit is waiting at the front door. Check Chats for the schedule." => "Não há visita marcada esperando na porta. Confira o horário em Conversas.",
            "Pipoca can only be offered after all six household groups have been sold." => "A venda do Pipoca só aparece depois da venda dos seis grupos de itens da casa.",
            "Household sales are offered at stress or urge 70 or above. Other actions remain available." => "As vendas ficam disponíveis quando o estresse ou o impulso chega a 70. As outras ações continuam disponíveis.",
            "No more care conversations can be recorded." => "Você atingiu o limite de conversas de apoio que o jogo consegue registrar.",
            "No more planning actions can be recorded." => "Você atingiu o limite de ações de planejamento que o jogo consegue registrar.",
            "Choose a whole-real daily cap of R$0 or more." => "Escolha um limite diário em reais inteiros, a partir de R$0.",
            "Start a New Game or Continue a saved campaign first." => "Inicie um novo jogo ou continue uma campanha salva.",
            _ if error.ends_with(" has already been sold. Sales are permanent.") =>
                "Este item já foi vendido. As vendas são definitivas.",
            _ => "Não foi possível realizar essa ação.",
        }.into()
    }

    pub fn campaign_notice<'a>(&self, notice: &'a str) -> &'a str {
        if self.language == Language::English || notice.is_empty() {
            return notice;
        }
        match notice {
            "No campaign to continue. Choose New Game." => {
                "Não há campanha para continuar. Escolha Novo jogo."
            }
            "Confirm New Game before replacing the existing campaign." => {
                "Confirme Novo jogo antes de substituir a campanha existente."
            }
            "There is no campaign to save." => "Não há campanha para salvar.",
            _ if notice.starts_with("Saved campaign could not be loaded:")
                || notice.starts_with("The unreadable existing snapshot") =>
            {
                "Não foi possível carregar a campanha. O jogo preservou o arquivo existente. Confirme Novo jogo para substituí-lo."
            }
            _ if notice.starts_with("Progress is kept only for this session") => {
                "O progresso está disponível só nesta sessão. Tente salvar novamente antes de fechar o jogo."
            }
            _ => "Não foi possível carregar ou salvar a campanha. Tente novamente.",
        }
    }

    pub fn quality_label<'a>(&self, label: &'a str) -> &'a str {
        match label {
            "Low" => self.choose(label, "Baixa"),
            "Medium" => self.choose(label, "Média"),
            "High" => self.choose(label, "Alta"),
            "Custom" => self.choose(label, "Personalizada"),
            _ => label,
        }
    }

    pub fn graphics_notice<'a>(&self, notice: &'a str) -> &'a str {
        if self.language == Language::English || notice.is_empty() {
            return notice;
        }
        match notice {
            "Saved graphics settings were invalid. High quality restored." => {
                "As configurações gráficas salvas eram inválidas. A qualidade alta foi restaurada."
            }
            "Local storage unavailable. Settings can still be used this session." => {
                "O armazenamento local está indisponível. As configurações valem para esta sessão."
            }
            _ => {
                "As configurações valem apenas para esta sessão. O armazenamento local está indisponível."
            }
        }
    }

    pub fn text(&self, key: &'static str) -> &'static str {
        match (self.language, key) {
            (Language::Portuguese, "continue") => "Continuar",
            (Language::Portuguese, "new_game") => "Novo jogo",
            (Language::Portuguese, "settings") => "Configurações",
            (Language::Portuguese, "phone_history") => "Histórico",
            (Language::Portuguese, "phone_betting") => "Apostas",
            (Language::Portuguese, "phone_support") => "Apoio",
            (Language::Portuguese, "phone_chats") => "Conversas",
            (Language::Portuguese, "language") => "Idioma",
            (Language::Portuguese, "english") => "Inglês",
            (Language::Portuguese, "portuguese") => "Português",
            (Language::Portuguese, "keep") => "Manter campanha atual",
            (Language::Portuguese, "replace") => "Substituir / Novo jogo",
            (Language::Portuguese, "retry") => "Tentar salvar campanha novamente",
            (Language::Portuguese, "loading_opening") => {
                "Carregando abertura…\nEsc: voltar ao menu principal"
            }
            (Language::Portuguese, "skip") => "Segure Enter para pular",
            (Language::Portuguese, "skip_wake") => "Pressione Enter para pular",
            (Language::Portuguese, "sleep") => "Dormi assistindo TV de novo…",
            (Language::Portuguese, "work") => "Preciso parar de trabalhar até tão tarde…",
            (Language::Portuguese, "work_at_night") => "Está tarde demais, talvez amanhã...",
            (Language::Portuguese, "too_hungry") => "Estou com muita fome. Preciso comer primeiro.",
            (Language::Portuguese, "missed_call") => {
                "Poxa, perdi a ligação com {name}… Não posso dar um vacilo desses de novo."
            }
            (Language::Portuguese, "landlord_knock") => "Toc, toc, toc!",
            (Language::Portuguese, "landlord_speaker") => "Proprietária do apartamento",
            (Language::Portuguese, "landlord_bills") => {
                "Alex? Os boletos estão em cima da mesa da sala. Pague cada um até o fim do dia indicado. Se o aluguel ficar em aberto, você vai ter que sair do apartamento."
            }
            (Language::Portuguese, "bill_water") => "Água",
            (Language::Portuguese, "bill_electricity") => "Luz",
            (Language::Portuguese, "bill_rent") => "Aluguel",
            (Language::Portuguese, "landlord_hunger") => {
                "Ah, e vê se não morre de fome. Tem comida na geladeira e dá pra pedir comida pronta."
            }
            (Language::Portuguese, "landlord_boredom") => {
                "Se bater o tédio, liga a TV. Nem que seja só pra ver como anda o placar."
            }
            (Language::Portuguese, "landlord_stress") => {
                "Cuidado com o estresse, hein? Até dar uma geral no apartamento pode ajudar."
            }
            (Language::Portuguese, "landlord_urge") => {
                "E fica de olho na vontade de apostar. Quando der aquela coceira de fazer só mais uma, vale fazer uma pausa."
            }
            (Language::Portuguese, "landlord_relationships") => {
                "Lembra de falar com o pessoal também. Uma ligação já ajuda a manter contato. Pode pegar o celular quando quiser."
            }
            (Language::Portuguese, "dialogue_continue") => "Enter ou clique para continuar",
            (Language::Portuguese, "marina_message") => {
                "estou aqui para qualquer coisa, você sabe disso né?"
            }
            (Language::Portuguese, "rafa_message") => "ei mano você tá bem?",
            (Language::Portuguese, "therapy_session") => "Sessão de terapia · 4 PA",
            (Language::Portuguese, "phone_hint") => {
                "Pressione [Tab] para pegar ou guardar o celular."
            }
            (Language::Portuguese, "bills") => "Mas o que ganho mal dá para pagar as contas.",
            (Language::Portuguese, "way") => "Vou encontrar um jeito.",
            (Language::Portuguese, "paused") => "PAUSADO",
            (Language::Portuguese, "resume") => "RETOMAR",
            (Language::Portuguese, "main_menu") => "MENU PRINCIPAL",
            (Language::Portuguese, "settings_upper") => "CONFIGURAÇÕES",
            (Language::Portuguese, "pause_hint") => {
                "Selecione Retomar para continuar. Esc novamente libera o mouse."
            }
            (Language::Portuguese, "hunger") => "Fome",
            (Language::Portuguese, "boredom") => "Tédio",
            (Language::Portuguese, "stress") => "Estresse",
            (Language::Portuguese, "urge") => "Impulso",
            (Language::Portuguese, "relationships") => "Relacionamentos",
            (Language::Portuguese, "actions_left") => "ações restantes",
            (Language::Portuguese, "morning") => "Manhã",
            (Language::Portuguese, "afternoon") => "Tarde",
            (Language::Portuguese, "evening") => "Noite",
            (Language::Portuguese, "night") => "Noite",
            (Language::Portuguese, "day") => "Dia",
            (_, "continue") => "Continue",
            (_, "new_game") => "New Game",
            (_, "settings") => "Settings",
            (_, "phone_history") => "History",
            (_, "phone_betting") => "Betting",
            (_, "phone_support") => "Support",
            (_, "phone_chats") => "Chats",
            (_, "language") => "Language",
            (_, "english") => "English",
            (_, "portuguese") => "Portuguese",
            (_, "keep") => "Keep existing campaign",
            (_, "replace") => "Replace / New Game",
            (_, "retry") => "Retry campaign save",
            (_, "loading_opening") => "Loading opening…\nEscape: return to title",
            (_, "skip") => "Hold Enter to skip",
            (_, "skip_wake") => "Press Enter to skip",
            (_, "sleep") => "I've fallen asleep watching TV again...",
            (_, "work") => "I gotta stop working late hours...",
            (_, "work_at_night") => "It's too late, maybe tomorrow...",
            (_, "too_hungry") => "Too hungry. Eat something first.",
            (_, "missed_call") => {
                "Damn, I missed my call with {name}… I can't keep slipping up like this."
            }
            (_, "landlord_knock") => "Knock, knock, knock!",
            (_, "landlord_speaker") => "Landlady",
            (_, "landlord_bills") => {
                "Alex? Your bills are on the living-room table. Pay each one by the end of its due day. If the rent goes unpaid, you'll have to move out."
            }
            (_, "bill_water") => "Water",
            (_, "bill_electricity") => "Electricity",
            (_, "bill_rent") => "Rent",
            (_, "landlord_hunger") => {
                "Oh, and don't go hungry. There's food in the fridge, and you can order delivery."
            }
            (_, "landlord_boredom") => {
                "If you get bored, turn on the TV. You can always check the score."
            }
            (_, "landlord_stress") => {
                "Keep an eye on your stress, all right? A little cleaning can help."
            }
            (_, "landlord_urge") => {
                "Watch that urge to bet, too. If you catch yourself itching for just one more, take a break."
            }
            (_, "landlord_relationships") => {
                "And check in with your friends. A quick call goes a long way. You can pick up your phone whenever you want."
            }
            (_, "dialogue_continue") => "Press Enter or click to continue",
            (_, "marina_message") => "I'm here if you need anything. You know that, right?",
            (_, "rafa_message") => "Hey man, you okay?",
            (_, "therapy_session") => "Therapy session · 4 PA",
            (_, "phone_hint") => "Press [Tab] to pick up or put away your phone.",
            (_, "bills") => "But what I make is barely enough to cover the bills.",
            (_, "way") => "I'll find a way.",
            (_, "paused") => "PAUSED",
            (_, "resume") => "RESUME",
            (_, "settings_upper") => "SETTINGS",
            (_, "main_menu") => "MAIN MENU",
            (_, "pause_hint") => "Select Resume to continue. Esc again releases mouse.",
            (_, "hunger") => "Hunger",
            (_, "boredom") => "Boredom",
            (_, "stress") => "Stress",
            (_, "urge") => "Urge",
            (_, "relationships") => "Relationships",
            (_, "actions_left") => "actions left",
            (_, "morning") => "Morning",
            (_, "day") => "Day",
            (_, "afternoon") => "Afternoon",
            (_, "evening") => "Evening",
            (_, "night") => "Night",
            (_, other) => other,
        }
    }

    pub fn period(&self, label: &str) -> String {
        match label {
            "Morning" => self.text("morning"),
            "Afternoon" => self.text("afternoon"),
            "Evening" => self.text("evening"),
            "Night" => self.text("night"),
            _ => label,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_language_tags_restore_with_portuguese_as_the_fallback() {
        for language in [Language::English, Language::Portuguese] {
            let json = serde_json::to_string(language.tag()).unwrap();
            assert_eq!(Localization::from_saved_language(Some(&json)).language, language);
        }
        for saved in [None, Some(r#""fr""#), Some("invalid JSON")] {
            assert_eq!(Localization::from_saved_language(saved).language, Language::Portuguese);
        }
    }
}
