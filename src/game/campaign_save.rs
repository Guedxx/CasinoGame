//! One campaign snapshot, separate from graphics preferences.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::campaign::{Action, Campaign, CommitmentStatus, Contact};

pub const STORAGE_KEY: &str = "casino-game.campaign.v1";
const SAVE_VERSION: u32 = 1;
const MAX_SAVE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    version: u32,
    checksum: u64,
    campaign: Campaign,
}

// Detect accidental corruption; the save is local data, not an anti-cheat boundary.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub fn encode_campaign(campaign: &Campaign) -> Result<String, String> {
    campaign.validate()?;
    let payload = serde_json::to_vec(campaign).map_err(|error| error.to_string())?;
    let json = serde_json::to_string(&Snapshot {
        version: SAVE_VERSION,
        checksum: checksum(&payload),
        campaign: campaign.clone(),
    })
    .map_err(|error| error.to_string())?;
    if json.len() > MAX_SAVE_BYTES {
        return Err("Campaign snapshot is too large to save.".into());
    }
    Ok(json)
}

pub fn decode_campaign(json: &str) -> Result<Campaign, String> {
    if json.len() > MAX_SAVE_BYTES {
        return Err("Campaign snapshot is too large to load.".into());
    }
    let mut snapshot: Snapshot = serde_json::from_str(json)
        .map_err(|error| format!("Invalid campaign snapshot: {error}"))?;
    if snapshot.version != SAVE_VERSION {
        return Err(format!(
            "Campaign save version {} is unsupported.",
            snapshot.version
        ));
    }
    let payload = serde_json::to_vec(&snapshot.campaign).map_err(|error| error.to_string())?;
    if checksum(&payload) != snapshot.checksum {
        return Err("Campaign snapshot failed its corruption check.".into());
    }
    snapshot.campaign.validate()?;
    snapshot.campaign.update_unpaid_bills();
    Ok(snapshot.campaign)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotState {
    Missing,
    Available,
    Protected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    SessionOnly,
}

/// The only live campaign owner. UI reads `campaign` and displays `notice`.
#[derive(Resource)]
pub struct CampaignStore {
    campaign: Option<Campaign>,
    snapshot_state: SnapshotState,
    pub notice: String,
    pub(super) feedback: ActionFeedback,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ActionFeedback {
    pub attributes: [i16; 5],
    pub money: i64,
    pub missed_call: Option<Contact>,
}

impl Default for CampaignStore {
    fn default() -> Self {
        let mut store = Self {
            campaign: None,
            snapshot_state: SnapshotState::Missing,
            notice: String::new(),
            feedback: ActionFeedback::default(),
        };
        let _ = store.load();
        store
    }
}

impl CampaignStore {
    #[cfg(test)]
    pub(crate) fn for_test(campaign: Option<Campaign>, snapshot_state: SnapshotState) -> Self {
        Self {
            campaign,
            snapshot_state,
            notice: String::new(),
            feedback: ActionFeedback::default(),
        }
    }

    pub fn campaign(&self) -> Option<&Campaign> {
        self.campaign.as_ref()
    }

    pub fn snapshot_state(&self) -> SnapshotState {
        self.snapshot_state
    }

    pub fn needs_replace_confirmation(&self) -> bool {
        self.campaign.is_some() || self.snapshot_state != SnapshotState::Missing
    }

    /// Loading never writes or removes data; failures retain the current session.
    pub fn load(&mut self) -> Result<bool, String> {
        self.load_result(read_snapshot())
    }

    fn load_result(&mut self, result: Result<Option<String>, String>) -> Result<bool, String> {
        let result = result.and_then(|json| json.map(|json| decode_campaign(&json)).transpose());
        match result {
            Ok(Some(campaign)) => {
                self.campaign = Some(campaign);
                self.snapshot_state = SnapshotState::Available;
                self.notice.clear();
                self.feedback = ActionFeedback::default();
                Ok(true)
            }
            Ok(None) => {
                self.snapshot_state = SnapshotState::Missing;
                self.notice = if self.campaign.is_some() {
                    "Progress is kept only for this session. No campaign snapshot was found. Retry saving before closing.".into()
                } else {
                    String::new()
                };
                Ok(false)
            }
            Err(error) => {
                self.snapshot_state = SnapshotState::Protected;
                self.notice = format!(
                    "Saved campaign could not be loaded: {error} The existing save is preserved. Confirm New Game to replace it."
                );
                warn!("{}", self.notice);
                Err(self.notice.clone())
            }
        }
    }

    /// The caller obtains explicit UI confirmation before passing `true`.
    pub fn new_campaign(&mut self, replacement_confirmed: bool) -> Result<SaveStatus, String> {
        if self.needs_replace_confirmation() && !replacement_confirmed {
            return Err("Confirm New Game before replacing the existing campaign.".into());
        }
        let seed = new_seed()?;
        self.campaign = Some(Campaign::new(seed));
        self.snapshot_state = SnapshotState::Missing;
        self.feedback = ActionFeedback::default();
        Ok(self.autosave_with(write_snapshot))
    }

    /// A save failure never turns a settled action into a rejected action.
    pub fn apply(&mut self, action: Action) -> Result<SaveStatus, String> {
        self.apply_with(action, write_snapshot)
    }

    fn apply_with(
        &mut self,
        action: Action,
        writer: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<SaveStatus, String> {
        let campaign = self
            .campaign
            .as_mut()
            .ok_or("Start a New Game or Continue a saved campaign first.")?;
        let before = campaign.attributes.values();
        let balance = campaign.balance_cents;
        let scheduled_call = campaign.commitments.iter().position(|plan| {
            plan.day == campaign.clock.day
                && plan.status == CommitmentStatus::Scheduled
                && campaign.uses_phone_for(plan.contact)
        });
        campaign.apply(action)?;
        let after = campaign.attributes.values();
        for (index, delta) in self.feedback.attributes.iter_mut().enumerate() {
            *delta += i16::from(after[index]) - i16::from(before[index]);
        }
        self.feedback.money += campaign.balance_cents - balance;
        if let Some(plan) = scheduled_call.map(|index| &campaign.commitments[index])
            && plan.status == CommitmentStatus::Missed
        {
            self.feedback.missed_call = Some(plan.contact);
        }
        Ok(self.autosave_with(writer))
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.save_with(write_snapshot)
    }

    pub(super) fn finish_landlord_intro(&mut self) {
        if let Some(campaign) = &mut self.campaign
            && campaign.landlord_intro_due()
        {
            campaign.landlord_intro_seen = Some(true);
            let _ = self.save();
        }
    }

    fn autosave_with(&mut self, writer: impl FnOnce(&str) -> Result<(), String>) -> SaveStatus {
        if self.save_with(writer).is_ok() {
            SaveStatus::Saved
        } else {
            SaveStatus::SessionOnly
        }
    }

    fn save_with(&mut self, writer: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
        let result = if self.snapshot_state == SnapshotState::Protected {
            Err("The unreadable existing snapshot is protected. Confirm New Game before replacing it.".into())
        } else {
            self.campaign
                .as_ref()
                .ok_or_else(|| "There is no campaign to save.".to_string())
                .and_then(encode_campaign)
                .and_then(|json| writer(&json))
        };
        match result {
            Ok(()) => {
                self.snapshot_state = SnapshotState::Available;
                self.notice.clear();
                Ok(())
            }
            Err(error) => {
                self.notice = format!(
                    "Progress is kept only for this session: {error} Retry saving before closing."
                );
                warn!("{}", self.notice);
                Err(self.notice.clone())
            }
        }
    }
}

/// New Game reset hook for player controls, overlays and state-derived household visuals.
#[derive(Event)]
pub(crate) struct CampaignReplaced;

pub struct CampaignPlugin;

impl Plugin for CampaignPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampaignStore>();
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or("Browser local storage is disabled.".into())
}

#[cfg(target_arch = "wasm32")]
fn read_snapshot() -> Result<Option<String>, String> {
    storage()?
        .get_item(STORAGE_KEY)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(target_arch = "wasm32")]
fn write_snapshot(json: &str) -> Result<(), String> {
    storage()?
        .set_item(STORAGE_KEY, json)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(target_arch = "wasm32")]
fn new_seed() -> Result<u64, String> {
    let mut bytes = [0; 8];
    web_sys::window()
        .ok_or("No browser window")?
        .crypto()
        .map_err(|error| format!("Browser randomness is unavailable: {error:?}"))?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|error| format!("Could not seed the campaign: {error:?}"))?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(not(target_arch = "wasm32"))]
fn new_seed() -> Result<u64, String> {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("Cannot seed a new campaign: {error}"))?;
    Ok(time.as_nanos() as u64 ^ u64::from(std::process::id()).rotate_left(32))
}

#[cfg(not(target_arch = "wasm32"))]
fn snapshot_path() -> Result<std::path::PathBuf, String> {
    use std::{env, path::PathBuf};
    let root = if cfg!(target_os = "windows") {
        env::var_os("APPDATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME")
            .filter(|path| !path.is_empty())
            .map(|path| PathBuf::from(path).join("Library/Application Support"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .filter(|path| !path.is_empty())
                    .map(|path| PathBuf::from(path).join(".config"))
            })
    }
    .ok_or("No user configuration directory is available.")?;
    Ok(root.join("CasinoGame").join(format!("{STORAGE_KEY}.json")))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_snapshot() -> Result<Option<String>, String> {
    read_file(&snapshot_path()?)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &std::path::Path) -> Result<Option<String>, String> {
    use std::io::Read;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut json = String::new();
    file.take(MAX_SAVE_BYTES as u64 + 1)
        .read_to_string(&mut json)
        .map_err(|error| error.to_string())?;
    if json.len() > MAX_SAVE_BYTES {
        return Err("Campaign snapshot is too large to load.".into());
    }
    Ok(Some(json))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_snapshot(json: &str) -> Result<(), String> {
    write_file(&snapshot_path()?, json)
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &std::path::Path, json: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(path.parent().ok_or("Invalid campaign save path.")?)
        .map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(json.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::campaign::{BillKind, CampaignStage, Ending, INITIAL_BALANCE_CENTS};

    fn empty_store() -> CampaignStore {
        CampaignStore::for_test(None, SnapshotState::Missing)
    }

    #[test]
    fn snapshot_roundtrip_preserves_rng_and_ended_state() {
        let mut original = Campaign::new(42);
        original.apply(Action::Wait).unwrap();
        original.rng.sample_below(100).unwrap();
        let mut restored = decode_campaign(&encode_campaign(&original).unwrap()).unwrap();
        assert_eq!(restored, original);
        for _ in 0..20 {
            assert_eq!(
                restored.rng.sample_below(100).unwrap(),
                original.rng.sample_below(100).unwrap()
            );
        }
        original.stage = CampaignStage::Ended(Ending::FinalDay);
        assert_eq!(
            decode_campaign(&encode_campaign(&original).unwrap())
                .unwrap()
                .stage,
            original.stage
        );
    }

    #[test]
    fn legacy_snapshot_keeps_its_checksum_calendar_and_opening_balance() {
        let json = encode_campaign(&Campaign::legacy(42)).unwrap();
        // Original v1 checksum for a new campaign with seed 42.
        assert!(json.starts_with(r#"{"version":1,"checksum":11347336620758525511,"campaign":"#));
        let restored = decode_campaign(&json).unwrap();
        assert_eq!((restored.last_day(), restored.balance_cents), (10, 12_000));
        assert_eq!(restored.bills[1].due_day, 7);

        let json = encode_campaign(&Campaign::previous_economy(42)).unwrap();
        assert!(json.starts_with(r#"{"version":1,"checksum":10692789218434743493,"campaign":"#));
        let mut restored = decode_campaign(&json).unwrap();
        assert_eq!((restored.last_day(), restored.balance_cents), (4, 36_000));
        restored.apply(Action::Work).unwrap();
        assert_eq!(restored.balance_cents, 44_000);
    }

    #[test]
    fn saved_unpaid_bills_get_new_prices_and_paid_bills_keep_their_ledger() {
        let mut campaign = Campaign::new(42);
        for action in [Action::Work, Action::Sleep, Action::AcknowledgeReport, Action::Work] {
            campaign.apply(action).unwrap();
        }
        for (kind, old_price, new_price) in [
            (BillKind::Water, 6_000, 10_000),
            (BillKind::Electricity, 9_000, 10_000),
            (BillKind::Rent, 11_000, 12_000),
            (BillKind::Rent, 13_000, 12_000),
        ] {
            let mut old = campaign.clone();
            old.bills.iter_mut().find(|bill| bill.kind == kind).unwrap().amount_cents = old_price;
            let mut restored = decode_campaign(&encode_campaign(&old).unwrap()).unwrap();
            assert_eq!(restored, campaign);
            restored.apply(Action::PayBill(kind)).unwrap();
            assert_eq!(restored.balance_cents, campaign.balance_cents - new_price);

            old.apply(Action::PayBill(kind)).unwrap();
            assert_eq!(decode_campaign(&encode_campaign(&old).unwrap()).unwrap(), old);
        }
    }

    #[test]
    fn delivery_pay_cap_preserves_existing_payments_above_eighty() {
        let mut old = Campaign::new(42);
        old.apply(Action::Work).unwrap();
        old.balance_cents = INITIAL_BALANCE_CENTS + 9_000;
        let payment = old.ledger.last_mut().unwrap();
        payment.amount_cents = 9_000;
        payment.balance_after_cents = old.balance_cents;

        let mut restored = decode_campaign(&encode_campaign(&old).unwrap()).unwrap();
        assert_eq!(restored, old);
        restored.apply(Action::Sleep).unwrap();
        restored.apply(Action::AcknowledgeReport).unwrap();
        restored.apply(Action::Work).unwrap();
        assert_eq!(restored.reports[0].finances.work_cents, 9_000);
        assert!((4_000..=7_000).contains(&restored.ledger.last().unwrap().amount_cents));
    }

    #[test]
    fn routine_report_and_next_day_are_each_autosaved() {
        let mut store = CampaignStore::for_test(Some(Campaign::new(5)), SnapshotState::Missing);
        let mut pay = 0;
        store
            .apply_with(Action::Work, |json| {
                let saved = decode_campaign(json).unwrap();
                pay = saved.ledger[0].amount_cents;
                assert!((4_000..=7_000).contains(&pay));
                assert_eq!(saved.balance_cents, INITIAL_BALANCE_CENTS + pay);
                assert_eq!(saved.worked_days, [1]);
                Ok(())
            })
            .unwrap();
        store
            .apply_with(Action::Sleep, |json| {
                let saved = decode_campaign(json).unwrap();
                assert_eq!(saved.stage, CampaignStage::DailyReport);
                assert_eq!(saved.clock.day, 1);
                assert_eq!(saved.reports[0].pa_used, 2);
                assert_eq!(
                    saved.reports[0].finances.closing_cents,
                    INITIAL_BALANCE_CENTS + pay
                );
                Ok(())
            })
            .unwrap();
        store
            .apply_with(Action::AcknowledgeReport, |json| {
                let saved = decode_campaign(json).unwrap();
                assert_eq!(saved.stage, CampaignStage::Playing);
                assert_eq!(saved.clock.day, 2);
                assert_eq!(saved.clock.pa_spent, 0);
                assert_eq!(saved.reports.len(), 1);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_version_corruption_unknown_fields_and_invalid_state_are_rejected() {
        let campaign = Campaign::new(42);
        let json = encode_campaign(&campaign).unwrap();
        assert!(decode_campaign(&json.replace("\"version\":1", "\"version\":2")).is_err());
        assert!(decode_campaign(&json.replace("\"stress\":30", "\"stress\":31")).is_err());
        assert!(
            decode_campaign(
                &json.replace("\"stage\":\"Playing\"", "\"extra\":0,\"stage\":\"Playing\"")
            )
            .is_err()
        );
        assert!(decode_campaign(&json[..json.len() - 1]).is_err());
        let mut invalid = campaign.clone();
        invalid.balance_cents = -1;
        assert!(encode_campaign(&invalid).is_err());
        let payload = serde_json::to_vec(&invalid).unwrap();
        let forged = serde_json::to_string(&Snapshot {
            version: 1,
            checksum: checksum(&payload),
            campaign: invalid,
        })
        .unwrap();
        assert!(decode_campaign(&forged).is_err());
    }

    #[test]
    fn loading_is_read_only_and_invalid_saves_require_explicit_replacement() {
        let mut store = empty_store();
        assert!(!store.load_result(Ok(None)).unwrap());
        assert!(store.campaign().is_none());
        assert!(!store.needs_replace_confirmation());
        assert!(store.load_result(Ok(Some("broken".into()))).is_err());
        assert_eq!(store.snapshot_state(), SnapshotState::Protected);
        assert!(store.needs_replace_confirmation());
        assert!(store.new_campaign(false).is_err());
        assert!(store.campaign().is_none());
        store.campaign = Some(Campaign::new(42));
        assert!(
            store
                .save_with(|_| panic!("Protected snapshot must not be overwritten"))
                .is_err()
        );
        let before = store.campaign().cloned();
        assert!(store.load_result(Err("permission denied".into())).is_err());
        assert_eq!(store.campaign().cloned(), before);
    }

    #[test]
    fn committed_actions_autosave_and_failures_remain_committed_with_notice() {
        let mut store = empty_store();
        store.campaign = Some(Campaign::new(42));
        let status = store
            .apply_with(Action::Wait, |json| {
                assert_eq!(decode_campaign(json).unwrap().clock.pa_spent, 1);
                Ok(())
            })
            .unwrap();
        assert_eq!(status, SaveStatus::Saved);
        assert!(store.notice.is_empty());
        let status = store
            .apply_with(Action::Wait, |_| Err("disk full".into()))
            .unwrap();
        assert_eq!(status, SaveStatus::SessionOnly);
        assert_eq!(store.campaign().unwrap().clock.pa_spent, 2);
        assert!(store.notice.contains("disk full"));
        store
            .save_with(|json| {
                assert_eq!(decode_campaign(json).unwrap().clock.pa_spent, 2);
                Ok(())
            })
            .unwrap();
        assert!(store.notice.is_empty());
        for _ in 0..4 {
            store.apply_with(Action::Wait, |_| Ok(())).unwrap();
        }
        let before = store.campaign().cloned();
        assert!(
            store
                .apply_with(Action::Wait, |_| panic!("Rejected actions must not save"))
                .is_err()
        );
        assert_eq!(store.campaign().cloned(), before);
    }

    #[test]
    fn missing_snapshot_keeps_unsaved_progress_and_its_warning() {
        let mut store = empty_store();
        store.campaign = Some(Campaign::new(42));
        assert_eq!(
            store
                .apply_with(Action::Wait, |_| Err("disk full".into()))
                .unwrap(),
            SaveStatus::SessionOnly
        );
        let before = store.campaign().cloned();
        assert!(!store.load_result(Ok(None)).unwrap());
        assert_eq!(store.campaign().cloned(), before);
        assert_eq!(store.snapshot_state(), SnapshotState::Missing);
        assert!(store.notice.contains("only for this session"));
        assert!(store.notice.contains("Retry saving"));
        store.save_with(|_| Ok(())).unwrap();
        assert!(store.notice.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_save_replaces_atomically_and_preserves_snapshot_on_write_failure() {
        let directory = std::env::temp_dir().join(format!(
            "casino-campaign-test-{}-{}",
            std::process::id(),
            new_seed().unwrap()
        ));
        let path = directory.join("campaign.json");
        assert_eq!(read_file(&path).unwrap(), None);
        let original = encode_campaign(&Campaign::new(42)).unwrap();
        write_file(&path, &original).unwrap();
        assert_eq!(
            read_file(&path).unwrap().as_deref(),
            Some(original.as_str())
        );
        let mut next = Campaign::new(42);
        next.apply(Action::Wait).unwrap();
        let replacement = encode_campaign(&next).unwrap();
        write_file(&path, &replacement).unwrap();
        assert_eq!(
            read_file(&path).unwrap().as_deref(),
            Some(replacement.as_str())
        );
        std::fs::create_dir(path.with_extension("tmp")).unwrap();
        assert!(write_file(&path, &original).is_err());
        assert_eq!(
            read_file(&path).unwrap().as_deref(),
            Some(replacement.as_str())
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn betting_autosaves_committed_result_and_failed_save_never_recharges_on_retry() {
        use super::super::campaign::{BetChoice, BetOutcome};
        let mut store = CampaignStore::for_test(Some(Campaign::new(5)), SnapshotState::Missing);
        let status = store
            .apply_with(
                Action::Bet {
                    choice: BetChoice::RouletteRed,
                    stake_cents: 100,
                },
                |json| {
                    let c = decode_campaign(json).unwrap();
                    assert_eq!(c.betting.history.len(), 1);
                    assert_eq!(c.clock.pa_spent, 1);
                    assert!(matches!(
                        c.betting.history[0].outcome,
                        BetOutcome::RouletteRed | BetOutcome::RouletteBlack
                    ));
                    assert_eq!(c.betting.session.as_ref().unwrap().paid_rounds_remaining, 2);
                    Err("disk full".into())
                },
            )
            .unwrap();
        assert_eq!(status, SaveStatus::SessionOnly);
        let settled = store.campaign().unwrap().clone();
        store
            .save_with(|json| {
                assert_eq!(decode_campaign(json).unwrap(), settled);
                Ok(())
            })
            .unwrap();
        assert_eq!(store.campaign(), Some(&settled));
        assert!(store.notice.is_empty());
    }

    #[test]
    fn bill_extension_calls_and_visits_autosave_without_duplicate_payments() {
        use super::super::campaign::{BillKind, CommitmentStatus, Contact, LedgerKind};
        let mut c = Campaign::legacy(5);
        for _ in 1..4 {
            c.apply(Action::Work).unwrap();
            c.apply(Action::Sleep).unwrap();
            c.apply(Action::AcknowledgeReport).unwrap();
        }
        let mut store = CampaignStore::for_test(Some(c), SnapshotState::Missing);
        let mut snapshots = vec![];
        for action in [
            Action::PayBill(BillKind::Electricity),
            Action::NegotiateRent,
            Action::Wait,
            Action::Call(Contact::Marina),
            Action::ReceiveVisit(Contact::Rafa),
        ] {
            assert_eq!(
                store
                    .apply_with(action, |json| {
                        snapshots.push(decode_campaign(json).unwrap());
                        Ok(())
                    })
                    .unwrap(),
                SaveStatus::Saved
            );
            assert_eq!(store.campaign(), snapshots.last());
        }
        let settled = snapshots.last().unwrap();
        assert_eq!(settled.balance_cents, 26_000);
        assert_eq!(settled.clock.pa_spent, 4);
        assert!(settled.support.rent_negotiated);
        assert_eq!(settled.bills[1].due_day, 10);
        assert_eq!(settled.commitments[1].status, CommitmentStatus::Completed);
        assert_eq!(
            settled
                .ledger
                .iter()
                .filter(|row| row.kind == LedgerKind::BillPayment(BillKind::Electricity))
                .count(),
            1
        );
        assert!(
            store
                .apply_with(Action::PayBill(BillKind::Electricity), |_| panic!(
                    "A duplicate payment must not save"
                ))
                .is_err()
        );
        assert_eq!(store.campaign(), Some(settled));
    }

    #[test]
    fn support_actions_autosave_and_tools_persist_until_the_next_morning() {
        use super::super::campaign::BetChoice;
        let mut store = CampaignStore::for_test(Some(Campaign::new(5)), SnapshotState::Missing);
        let mut snapshot = String::new();
        for action in [
            Action::Bet {
                choice: BetChoice::RouletteRed,
                stake_cents: 100,
            },
            Action::CareConversation,
            Action::FinancialPlanning,
            Action::SetDailyCap(Some(200)),
            Action::SetDailyCap(Some(300)),
            Action::BlockBetting,
        ] {
            assert_eq!(
                store
                    .apply_with(action, |json| {
                        snapshot = json.into();
                        Ok(())
                    })
                    .unwrap(),
                SaveStatus::Saved
            );
            let saved = decode_campaign(&snapshot).unwrap();
            assert_eq!(store.campaign(), Some(&saved));
            store.load_result(Ok(Some(snapshot.clone()))).unwrap();
        }
        let before = store.campaign().unwrap().clone();
        assert_eq!(
            (before.support.care_actions, before.support.planning_actions),
            (1, 1)
        );
        assert_eq!(before.support.daily_cap_cents, Some(200));
        assert!(before.support.app_blocked_today);
        assert!(
            store
                .apply_with(
                    Action::Bet {
                        choice: BetChoice::Slots,
                        stake_cents: 100
                    },
                    |_| panic!("A blocked stake must not save")
                )
                .is_err()
        );
        assert_eq!(store.campaign(), Some(&before));
        assert_eq!(
            store
                .apply_with(Action::SetDailyCap(None), |_| Err("quota full".into()))
                .unwrap(),
            SaveStatus::SessionOnly
        );
        let committed = store.campaign().unwrap().clone();
        store
            .save_with(|json| {
                snapshot = json.into();
                Ok(())
            })
            .unwrap();
        store.load_result(Ok(Some(snapshot.clone()))).unwrap();
        assert_eq!(store.campaign(), Some(&committed));
        for action in [Action::Sleep, Action::AcknowledgeReport] {
            store
                .apply_with(action, |json| {
                    snapshot = json.into();
                    Ok(())
                })
                .unwrap();
            store.load_result(Ok(Some(snapshot.clone()))).unwrap();
            if action == Action::Sleep {
                assert!(store.campaign().unwrap().support.app_blocked_today);
                assert_eq!(store.campaign().unwrap().support.daily_cap_cents, Some(200));
            }
        }
        let next = store.campaign().unwrap();
        assert!(!next.support.app_blocked_today);
        assert!(next.support.daily_cap_cents.is_none());
        assert!(next.support.pending_cap.is_none());
        assert_eq!(
            (next.support.care_actions, next.support.planning_actions),
            (1, 1)
        );
        assert_eq!(next.betting.history, before.betting.history);
        assert_eq!(next.rng, before.rng);
    }
}
