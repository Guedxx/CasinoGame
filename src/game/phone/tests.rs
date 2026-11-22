use super::super::campaign_save::SnapshotState;
use super::*;

fn page_actions(phone: &Phone, campaign: &Campaign) -> Vec<(String, PhoneCommand)> {
    super::page_actions(phone, campaign, &Localization::ENGLISH)
}

fn home_reminder(campaign: &Campaign) -> String {
    super::home_reminder(campaign, &Localization::ENGLISH)
}

fn rendered_copy(page: PhonePage, campaign: &Campaign) -> String {
    rendered_copy_in_language(page, campaign, crate::localization::Language::English)
}

fn rendered_copy_in_language(
    page: PhonePage,
    campaign: &Campaign,
    language: crate::localization::Language,
) -> String {
    let mut app = app();
    app.insert_resource(Localization { language });
    app.insert_resource(CampaignStore::for_test(
        Some(campaign.clone()),
        SnapshotState::Protected,
    ));
    opened(&mut app, page);
    if page.is_game() {
        app.world_mut().resource_mut::<Phone>().settled_round = campaign.betting.history.last().cloned();
    }
    app.insert_resource(PhoneAssets {
        camera: Entity::PLACEHOLDER,
        target: Handle::default(),
        font: Handle::default(),
    });
    app.add_systems(PostUpdate, present);
    app.update();
    let mut texts = app.world_mut().query::<&Text>();
    texts
        .iter(app.world())
        .map(|text| text.0.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wager(app: &mut App, choice: BetChoice) {
    if choice != BetChoice::Slots {
        click_command(app, PhoneCommand::SelectBet(choice));
    }
    let stake_cents = app.world().resource::<Phone>().stake_cents;
    click_command(
        app,
        PhoneCommand::Apply(Action::Bet {
            choice,
            stake_cents,
        }),
    );
}

#[test]
fn phone_history_shows_the_daily_money_log_and_leaves_the_campaign_unchanged() {
    let mut c = campaign_after_landlord(5);
    c.apply(Action::Work).unwrap();
    let earned = c.ledger[0].amount_cents;
    c.apply(Action::OrderFood).unwrap();
    c.apply(Action::Sleep).unwrap();
    c.apply(Action::AcknowledgeReport).unwrap();
    let copy = rendered_copy(PhonePage::History, &c);
    assert!(copy.contains("Day 2\nReceived: R$0.00\nSpent: R$0.00"));
    let money = Localization::ENGLISH.reais(earned);
    assert!(copy.contains(&format!(
        "Day 1\nReceived: {money}\n  MeFood deliveries: {money}\nSpent: R$20.00\n  Food: R$20.00"
    )));
    assert!(!copy.contains("Relationships"));
    let mut app = app();
    app.insert_resource(CampaignStore::for_test(
        Some(c.clone()),
        SnapshotState::Protected,
    ));
    opened(&mut app, PhonePage::Home);
    key(&mut app, KeyCode::Digit1);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::History);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Home);
    assert_eq!(app.world().resource::<CampaignStore>().campaign(), Some(&c));
}

fn campaign_after_landlord(seed: u64) -> Campaign {
    let mut campaign = Campaign::new(seed);
    campaign.landlord_intro_seen = Some(true);
    campaign
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<launcher::Skin>()
        .init_resource::<ui::Art>()
        .insert_resource(State::new(GameState::Playing))
        .init_resource::<NextState<GameState>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Assets<Mesh>>()
        .add_message::<MouseWheel>()
        .add_message::<KeyboardInput>()
        .add_message::<PhoneAppClosed>()
        .insert_resource(CampaignStore::for_test(
            Some(campaign_after_landlord(9)),
            SnapshotState::Protected,
        ))
        .init_resource::<Phone>()
        .init_resource::<GameCursor>()
        .insert_resource(Localization::ENGLISH)
        .add_plugins(super::super::routine::RoutinePlugin)
        .add_systems(PreUpdate, input.after(OverlayInput))
        .add_systems(Update, (dispatch, end_session).chain().in_set(PhoneView))
        .add_observer(reset);
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        PrimaryWindow,
    ));
    app.world_mut().spawn(Player::default());
    app
}
fn amount_input(app: &mut App, key: KeyCode, text: Option<&str>, repeat: bool) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: match key {
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Enter => Key::Enter,
            _ => Key::Character(text.unwrap().into()),
        },
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat,
        window,
    });
}
fn key(app: &mut App, key: KeyCode) {
    let digit = match key {
        KeyCode::Digit0 => Some("0"),
        KeyCode::Digit1 => Some("1"),
        KeyCode::Digit2 => Some("2"),
        KeyCode::Digit3 => Some("3"),
        KeyCode::Digit4 => Some("4"),
        KeyCode::Digit5 => Some("5"),
        KeyCode::Digit6 => Some("6"),
        KeyCode::Digit7 => Some("7"),
        KeyCode::Digit8 => Some("8"),
        KeyCode::Digit9 => Some("9"),
        _ => None,
    };
    if digit.is_some() || matches!(key, KeyCode::Backspace | KeyCode::Enter) {
        amount_input(app, key, digit, false);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(key);
}
fn opened(app: &mut App, page: PhonePage) {
    let mut phone = app.world_mut().resource_mut::<Phone>();
    phone.opened = true;
    phone.raise = 1.0;
    phone.page = page;
    app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Phone);
}

#[test]
fn launcher_opens_only_four_apps_by_click_or_number_without_changing_campaign() {
    let mut app = app();
    app.insert_resource(PhoneAssets {
        camera: Entity::PLACEHOLDER,
        target: Handle::default(),
        font: Handle::default(),
    })
    .add_systems(Update, present);
    opened(&mut app, PhonePage::Home);
    app.update();
    let before = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    let buttons: Vec<_> = app
        .world_mut()
        .query::<&PhoneButton>()
        .iter(app.world())
        .map(|button| button.1)
        .collect();
    assert_eq!(buttons.len(), 4 + 3);
    for (page, key_code) in [
        (PhonePage::History, KeyCode::Digit1),
        (PhonePage::Betting, KeyCode::Digit2),
        (PhonePage::Support, KeyCode::Digit3),
        (PhonePage::Contacts, KeyCode::Digit4),
    ] {
        assert_eq!(
            buttons
                .iter()
                .filter(|&&cmd| cmd == PhoneCommand::Page(page))
                .count(),
            1
        );
        click_command(&mut app, PhoneCommand::Page(page));
        assert_eq!(app.world().resource::<Phone>().page, page);
        click_command(&mut app, PhoneCommand::Page(PhonePage::Home));
        key(&mut app, key_code);
        assert_eq!(app.world().resource::<Phone>().page, page);
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Home);
    }
    key(&mut app, KeyCode::Digit5);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Home);
    click_command(&mut app, PhoneCommand::Close);
    assert!(!app.world().resource::<Phone>().opened);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&before)
    );
}

#[test]
fn support_shows_relevant_sales_and_confirmations_explain_permanent_loss() {
    let mut c = campaign_after_landlord(5);
    let support = Phone {
        page: PhonePage::Support,
        ..default()
    };
    let sales = PhoneCommand::Page(PhonePage::Household);
    assert_eq!(
        page_actions(&support, &c)
            .into_iter()
            .map(|(_, command)| command)
            .collect::<Vec<_>>(),
        [
            PhoneCommand::Apply(Action::CareConversation),
            PhoneCommand::Apply(Action::FinancialPlanning),
            PhoneCommand::Apply(Action::TherapySession),
            PhoneCommand::Page(PhonePage::DailyCap),
            PhoneCommand::Apply(Action::BlockBetting),
        ]
    );
    let copy = rendered_copy(PhonePage::Support, &c);
    assert!(copy.contains("Therapy session · 4 PA"));
    for preview in ["Stress −10", "Stress −6", "Urge −30"] {
        assert!(!copy.contains(preview), "{preview}");
    }
    assert!(
        page_actions(
            &Phone {
                page: PhonePage::SaleOffers,
                ..default()
            },
            &c
        )
        .is_empty()
    );
    c.attributes.stress = 70;
    assert!(
        page_actions(&support, &c)
            .iter()
            .any(|(_, cmd)| *cmd == sales)
    );
    let choices = page_actions(
        &Phone {
            page: PhonePage::SaleOffers,
            ..default()
        },
        &c,
    );
    assert_eq!(choices.len(), 6);
    assert!(
        choices
            .iter()
            .all(|(_, cmd)| matches!(cmd, PhoneCommand::Page(PhonePage::ConfirmSale(_))))
    );
    for item in super::super::household::HOUSEHOLD_ITEMS
        .into_iter()
        .chain([HouseholdItem::Pipoca])
    {
        let choices = page_actions(
            &Phone {
                page: PhonePage::ConfirmSale(item),
                ..default()
            },
            &c,
        );
        let copy = rendered_copy(PhonePage::ConfirmSale(item), &c);
        assert!(
            copy.contains("permanent")
                && copy.contains("1 PA")
                && copy.contains("cannot be undone")
        );
        assert_eq!(choices[0].1, PhoneCommand::Page(PhonePage::SaleOffers));
        if item == HouseholdItem::Fridge {
            assert!(copy.contains("ALL 3 stored food portions"));
        }
        if item == HouseholdItem::Pipoca {
            assert!(copy.contains("ENDS THE CAMPAIGN"));
            assert_eq!(choices.len(), 1);
        }
    }
    assert_eq!(PhonePage::Household.back(), PhonePage::Support);
    assert_eq!(PhonePage::SaleOffers.back(), PhonePage::Household);
    c.apply(Action::Sell(HouseholdItem::Stove)).unwrap();
    c.attributes.stress = 0;
    assert!(
        page_actions(&support, &c)
            .iter()
            .any(|(_, cmd)| *cmd == sales)
    );
    assert_eq!(
        PhonePage::ConfirmSale(HouseholdItem::Tv).back(),
        PhonePage::SaleOffers
    );
}

#[test]
fn sales_stamp_appears_only_after_a_committed_sale_without_stress_text() {
    let mut c = campaign_after_landlord(5);
    let before = rendered_copy(PhonePage::Household, &c);
    assert!(before.contains("No sales yet"));
    assert!(!before.contains("TV"));
    assert!(!before.contains("SOLD"));
    assert!(!before.contains("Sell an item"));
    assert!(!before.to_lowercase().contains("stress"));
    c.attributes.stress = 70;
    let available = rendered_copy(PhonePage::Household, &c);
    assert!(available.contains("Sell an item"));
    assert!(!available.contains("SOLD"));
    c.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
    let after = rendered_copy(PhonePage::Household, &c);
    assert_eq!(after.matches("SOLD").count(), 1);
    assert!(after.contains("TV"));
    assert!(!after.contains("Fridge"));
    assert!(!after.to_lowercase().contains("stress"));
    let offers = rendered_copy(PhonePage::SaleOffers, &c);
    assert!(offers.contains("Fridge"));
    assert!(!offers.contains("TV"));
    assert!(!offers.contains("SOLD"));
    assert!(
        !page_actions(
            &Phone {
                page: PhonePage::SaleOffers,
                ..default()
            },
            &c
        )
        .iter()
        .any(|(_, command)| *command
            == PhoneCommand::Page(PhonePage::ConfirmSale(HouseholdItem::Tv)))
    );
}

#[test]
fn sale_navigation_cancel_held_keys_and_stale_commands_never_auto_confirm() {
    let mut app = app();
    let mut c = campaign_after_landlord(5);
    c.attributes.urge = 70;
    app.insert_resource(CampaignStore::for_test(
        Some(c.clone()),
        SnapshotState::Protected,
    ));
    opened(&mut app, PhonePage::Support);
    click_command(&mut app, PhoneCommand::Page(PhonePage::Household));
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Household);
    key(&mut app, KeyCode::Digit1);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::SaleOffers);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.update();
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::ConfirmSale(HouseholdItem::Tv)
    );
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    key(&mut app, KeyCode::Digit1);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::SaleOffers);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    // An old confirmation queued while the offers page is current is rejected.
    {
        let mut phone = app.world_mut().resource_mut::<Phone>();
        phone.pointer = Some(Vec2::ONE);
        phone.pending = Some((
            PhonePage::ConfirmSale(HouseholdItem::Tv),
            PhoneCommand::Apply(Action::Sell(HouseholdItem::Tv)),
        ));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    key(&mut app, KeyCode::Digit1);
    key(&mut app, KeyCode::Digit2);
    c.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Household);
    assert!(!app.world().resource::<CampaignStore>().notice.is_empty());
}

#[test]
fn fridge_confirm_destroys_only_disclosed_portions_and_dog_confirm_immediately_stops_phone() {
    let mut app = app();
    let mut c = campaign_after_landlord(5);
    c.attributes.stress = 70;
    app.insert_resource(CampaignStore::for_test(
        Some(c.clone()),
        SnapshotState::Protected,
    ));
    opened(&mut app, PhonePage::ConfirmSale(HouseholdItem::Fridge));
    key(&mut app, KeyCode::Digit1);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
        &c
    );
    key(&mut app, KeyCode::Digit2);
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::ConfirmSale(HouseholdItem::Fridge)
    );
    key(&mut app, KeyCode::Digit2);
    let stored = app.world().resource::<CampaignStore>().campaign().unwrap();
    assert_eq!(
        (
            stored.household.food_portions,
            stored.balance_cents,
            stored.clock.pa_spent
        ),
        (
            0,
            INITIAL_BALANCE_CENTS + HouseholdItem::Fridge.sale_cents(),
            1
        )
    );
    for item in super::super::household::HOUSEHOLD_ITEMS {
        c.apply(Action::Sell(item)).unwrap();
    }
    c.apply(Action::Sleep).unwrap();
    c.apply(Action::AcknowledgeReport).unwrap();
    c.attributes.stress = 0;
    c.attributes.urge = 0;
    app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
    opened(&mut app, PhonePage::Household);
    key(&mut app, KeyCode::Digit1);
    key(&mut app, KeyCode::Digit1);
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::ConfirmSale(HouseholdItem::Pipoca)
    );
    key(&mut app, KeyCode::Digit2);
    assert_eq!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .stage,
        CampaignStage::Ended(Ending::DogSale)
    );
    assert!(!app.world().resource::<Phone>().opened);
    assert!(app.world().resource::<GameplayOverlay>().campaign_modal);
}

#[test]
fn cursor_conversion_tracks_canvas_scale_dpr_resize_and_native_identity() {
    for (input, viewport) in [
        (Vec2::new(1280.0, 720.0), Vec2::new(1600.0, 900.0)),
        (Vec2::new(960.0, 540.0), Vec2::new(1200.0, 675.0)),
        (Vec2::new(960.0, 540.0), Vec2::new(1920.0, 1080.0)),
        (Vec2::new(960.0, 540.0), Vec2::new(960.0, 540.0)),
    ] {
        let normalized = Vec2::new(0.61, 0.42);
        assert!(
            scale_cursor(normalized * input, input, viewport)
                .unwrap()
                .distance(normalized * viewport)
                < 0.001
        );
    }
    assert!(scale_cursor(Vec2::ONE, Vec2::ZERO, Vec2::ONE).is_none());
    assert!(scale_cursor(Vec2::splat(f32::NAN), Vec2::ONE, Vec2::ONE).is_none());
}

#[test]
fn ray_maps_rotated_screen_corners_and_rejects_outside_behind_and_parallel() {
    let transform = Transform::from_xyz(0.12, -0.03, -0.8).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        0.15,
        -0.1,
        0.04,
    ));
    let screen = GlobalTransform::from(transform);
    for uv in [
        Vec2::splat(0.5),
        Vec2::new(0.01, 0.01),
        Vec2::new(0.99, 0.99),
    ] {
        let point = screen.transform_point(Vec3::new(
            (uv.x - 0.5) * SCREEN_METERS.x,
            (0.5 - uv.y) * SCREEN_METERS.y,
            0.0,
        ));
        let ray = Ray3d::new(Vec3::ZERO, Dir3::new(point).unwrap());
        assert!(
            screen_coordinates(ray, &screen)
                .unwrap()
                .distance(uv * SCREEN_SIZE)
                < 0.001
        );
    }
    for uv in [Vec2::new(-0.01, 0.5), Vec2::new(0.5, 1.01)] {
        let point = screen.transform_point(Vec3::new(
            (uv.x - 0.5) * SCREEN_METERS.x,
            (0.5 - uv.y) * SCREEN_METERS.y,
            0.0,
        ));
        assert!(
            screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::new(point).unwrap()), &screen)
                .is_none()
        );
    }
    let flat = GlobalTransform::from_translation(Vec3::NEG_Z);
    assert!(screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::Z), &flat).is_none());
    assert!(screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::X), &flat).is_none());
}

#[test]
fn landlord_dialogue_preserves_prepaid_betting_rounds_and_the_open_phone() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<CampaignStore>()
        .apply(Action::Bet {
            choice: BetChoice::Slots,
            stake_cents: 100,
        })
        .unwrap();
    opened(&mut app, PhonePage::Slots);
    app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Landlord {
        page: 1,
        resume_phone: true,
    });
    key(&mut app, KeyCode::Tab);
    assert!(app.world().resource::<Phone>().opened);
    let store = app.world().resource::<CampaignStore>();
    let session = store.campaign().unwrap().betting.session.as_ref().unwrap();
    assert_eq!(session.paid_rounds_remaining, 2);
}

#[test]
fn tab_browsing_escape_and_app_exit_never_spend_campaign_resources() {
    let mut app = app();
    let original = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    app.world_mut().resource_mut::<GameplayOverlay>().phone_hint = true;
    app.update();
    let mut text = app.world_mut().query::<&Text>();
    assert!(text.iter(app.world()).any(|text| text.0.contains("[Tab]")));
    key(&mut app, KeyCode::Tab);
    assert!(app.world().resource::<Phone>().opened);
    assert!(!text.iter(app.world()).any(|text| text.0.contains("[Tab]")));
    assert!(app.world().resource::<GameplayOverlay>().blocks_world());
    key(&mut app, KeyCode::Digit4);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Contacts);
    key(&mut app, KeyCode::KeyH);
    key(&mut app, KeyCode::Digit2);
    key(&mut app, KeyCode::Digit1);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Slots);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Betting);
    assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 0);
    key(&mut app, KeyCode::KeyH);
    assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 1);
    key(&mut app, KeyCode::Escape);
    assert!(!app.world().resource::<Phone>().opened);
    assert!(app.world().resource::<GameplayOverlay>().escape_consumed);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
}

#[test]
fn focus_loss_and_pause_preserve_page_without_exiting_the_app() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    key(&mut app, KeyCode::Tab);
    assert!(app.world().resource::<Phone>().opened);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Roulette);
    app.insert_resource(State::new(GameState::Menu));
    app.update();
    assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 0);
    app.insert_resource(State::new(GameState::Playing));
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Roulette);
    assert_eq!(
        app.world().resource::<GameplayOverlay>().active,
        Some(OverlayKind::Phone)
    );
}

#[test]
fn stale_dismissed_unfocused_paused_and_outside_presses_do_not_apply_care() {
    for scenario in 0..6 {
        let mut app = app();
        opened(&mut app, PhonePage::Support);
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pointer = Some(Vec2::splat(50.0));
            phone.pending = Some((
                if scenario == 0 {
                    PhonePage::Household
                } else {
                    PhonePage::Support
                },
                PhoneCommand::Apply(Action::CareConversation),
            ));
            if scenario == 1 {
                phone.pointer = None;
            }
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        match scenario {
            2 => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            3 => {
                let mut q = app
                    .world_mut()
                    .query_filtered::<&mut Window, With<PrimaryWindow>>();
                q.single_mut(app.world_mut()).unwrap().focused = false;
            }
            4 => {
                *app.world_mut().resource_mut::<NextState<GameState>>() =
                    NextState::Pending(GameState::Menu)
            }
            5 => {
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .reset(MouseButton::Left);
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .release(MouseButton::Left);
            }
            _ => {}
        }
        app.update();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original),
            "scenario {scenario}"
        );
    }
}

#[test]
fn leaving_screen_preserves_fresh_home_and_back_keyboard_navigation() {
    for (page, shortcut, expected) in [
        (PhonePage::DailyCap, KeyCode::KeyH, PhonePage::Home),
        (PhonePage::Roulette, KeyCode::Backspace, PhonePage::Betting),
    ] {
        let mut app = app();
        opened(&mut app, page);
        app.add_message::<PointerInput>()
            .insert_resource(PhoneAssets {
                camera: Entity::PLACEHOLDER,
                target: Handle::default(),
                font: Handle::default(),
            })
            .add_systems(PreUpdate, drive_pointer.after(input));
        app.world_mut().resource_mut::<Phone>().pointer = Some(Vec2::splat(50.0));
        key(&mut app, shortcut);
        assert_eq!(app.world().resource::<Phone>().page, expected);
        assert!(app.world().resource::<Phone>().pointer.is_none());
        assert_eq!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .clock
                .pa_spent,
            0
        );
    }
}

#[test]
fn outside_screen_cancels_held_virtual_pointer_and_never_reuses_its_position() {
    let mut app = app();
    opened(&mut app, PhonePage::Support);
    app.add_message::<PointerInput>()
        .insert_resource(PhoneAssets {
            camera: Entity::PLACEHOLDER,
            target: Handle::default(),
            font: Handle::default(),
        })
        .add_systems(PreUpdate, drive_pointer.after(input));
    app.world_mut().resource_mut::<Phone>().pointer = Some(Vec2::new(50.0, 150.0));
    app.world_mut().resource_mut::<Phone>().pending =
        Some((PhonePage::Support, PhoneCommand::Apply(Action::CareConversation)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    let mut cursor = bevy::ecs::message::MessageCursor::<PointerInput>::default();
    let events = app.world().resource::<Messages<PointerInput>>();
    assert!(
        cursor
            .read(events)
            .any(|event| matches!(event.action, PointerAction::Cancel))
    );
    assert!(app.world().resource::<Phone>().pointer.is_none());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(
        !cursor
            .read(app.world().resource::<Messages<PointerInput>>())
            .any(|event| matches!(
                event.action,
                PointerAction::Press(_) | PointerAction::Release(_)
            ))
    );
    assert_eq!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clock
            .pa_spent,
        0
    );
}

#[test]
fn attribute_feedback_shows_clamped_changes_and_waits_for_the_wager_reveal() {
    let mut app = app();
    let mut campaign = campaign_after_landlord(9);
    campaign.attributes.stress = 4;
    app.insert_resource(CampaignStore::for_test(
        Some(campaign),
        SnapshotState::Protected,
    ));
    opened(&mut app, PhonePage::Support);
    key(&mut app, KeyCode::Digit1);
    let mut text = app.world_mut().query::<(&Text, &TextColor, &ChildOf)>();
    for (change, value, is_gain) in [("-4", "0", false), ("+3", "33", true)] {
        let (_, color, parent) = text
            .iter(app.world())
            .find(|(t, _, _)| t.0 == change)
            .unwrap();
        let color = color.0.to_srgba();
        assert_eq!(color.green > color.red, is_gain);
        assert!(
            text.iter(app.world())
                .any(|(t, _, sibling)| t.0 == value && sibling == parent)
        );
    }
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(std::time::Duration::from_secs(3));
    app.update();
    assert!(
        !text
            .iter(app.world())
            .any(|(t, _, _)| matches!(t.0.as_str(), "-4" | "+3"))
    );

    opened(&mut app, PhonePage::Slots);
    let balance = app.world().resource::<CampaignStore>().campaign().unwrap().balance_cents;
    wager(&mut app, BetChoice::Slots);
    let after = app.world().resource::<CampaignStore>().campaign().unwrap().balance_cents;
    let net = after - balance;
    let money_delta = format!("{}{}", if net > 0 { "+" } else { "" }, Localization::ENGLISH.reais(net));
    assert!(app.world().resource::<Phone>().animating());
    assert!(!text.iter(app.world()).any(|(t, _, _)| t.0 == "+8"));
    assert!(!text.iter(app.world()).any(|(t, _, _)| t.0 == money_delta));
    click_command(&mut app, PhoneCommand::Skip);
    assert!(text.iter(app.world()).any(|(t, _, _)| t.0 == "+8"));
    let (_, color, _) = text.iter(app.world()).find(|(t, _, _)| t.0 == money_delta).unwrap();
    let color = color.0.to_srgba();
    assert_eq!(color.green > color.red, net > 0);
}

#[test]
fn care_uses_authoritative_actions_and_report_or_ending_blocks_phone() {
    let mut app = app();
    opened(&mut app, PhonePage::Support);
    key(&mut app, KeyCode::Digit1);
    let campaign = app.world().resource::<CampaignStore>().campaign().unwrap();
    assert_eq!(campaign.clock.pa_spent, 1);
    assert_eq!(campaign.support.care_actions, 1);
    assert_eq!(campaign.attributes.stress, 20);
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .notice
            .contains("session")
    );
    for stage in [
        CampaignStage::DailyReport,
        CampaignStage::Ended(Ending::HousingLoss),
    ] {
        let mut campaign = campaign_after_landlord(3);
        if stage == CampaignStage::DailyReport {
            campaign.apply(Action::Sleep).unwrap();
        } else {
            for _ in 1..=campaign.last_day() {
                campaign.apply(Action::Sleep).unwrap();
                campaign.apply(Action::AcknowledgeReport).unwrap();
            }
        }
        assert_eq!(campaign.stage, stage);
        app.insert_resource(CampaignStore::for_test(
            Some(campaign),
            SnapshotState::Protected,
        ));
        key(&mut app, KeyCode::Tab);
        assert!(!app.world().resource::<Phone>().opened);
        assert!(app.world().resource::<GameplayOverlay>().campaign_modal);
    }
}

#[test]
fn camera_animation_queries_initialize_and_disable_both_auxiliary_cameras() {
    let mut app = app();
    app.init_resource::<Assets<StandardMaterial>>()
        .insert_resource(PhoneAssets {
            camera: Entity::PLACEHOLDER,
            target: Handle::default(),
            font: Handle::default(),
        })
        .add_systems(Update, (tag_model, scroll, present, animate).chain());
    let view = app
        .world_mut()
        .spawn((PhoneCamera, Camera3d::default()))
        .id();
    let texture = app.world_mut().spawn((TextureCamera, Camera2d)).id();
    let world_camera = app
        .world_mut()
        .spawn((
            super::super::camera::MainCamera,
            Camera3d::default(),
            IsDefaultUiCamera,
        ))
        .id();
    app.world_mut()
        .spawn((PhoneRoot, Transform::default(), Visibility::Hidden));
    app.update();
    assert!(!app.world().get::<Camera>(view).unwrap().is_active);
    assert!(!app.world().get::<Camera>(texture).unwrap().is_active);
    assert!(app.world().get::<Camera>(world_camera).unwrap().is_active);
    opened(&mut app, PhonePage::Home);
    app.update();
    assert!(app.world().get::<Camera>(view).unwrap().is_active);
    assert!(app.world().get::<Camera>(texture).unwrap().is_active);
    app.insert_resource(State::new(GameState::Menu));
    app.update();
    assert!(!app.world().get::<Camera>(view).unwrap().is_active);
    assert!(!app.world().get::<Camera>(texture).unwrap().is_active);
    assert!(app.world().get::<Camera>(world_camera).unwrap().is_active);
}
fn click_command(app: &mut App, command: PhoneCommand) {
    {
        let mut phone = app.world_mut().resource_mut::<Phone>();
        phone.pointer = Some(Vec2::splat(50.0));
        phone.pending = Some((phone.page, command));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset(MouseButton::Left);
}

#[test]
fn typed_stake_consumes_digits_and_backspace_and_only_explicit_bet_settles() {
    let mut app = app();
    opened(&mut app, PhonePage::Football);
    let original = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    click_command(&mut app, PhoneCommand::EditStake);
    for digit in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Backspace,
    ] {
        key(&mut app, digit);
    }
    assert_eq!(
        app.world().resource::<Phone>().stake_entry.as_deref(),
        Some("12")
    );
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Football);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Phone>().stake_cents, 1200);
    assert!(app.world().resource::<Phone>().stake_entry.is_none());
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    key(&mut app, KeyCode::Digit2);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    click_command(
        &mut app,
        PhoneCommand::Apply(Action::Bet {
            choice: BetChoice::FootballDraw,
            stake_cents: 1200,
        }),
    );
    let c = app.world().resource::<CampaignStore>().campaign().unwrap();
    assert_eq!(c.betting.history.len(), 1);
    assert_eq!(c.betting.history[0].choice, BetChoice::FootballDraw);
    assert_eq!(c.betting.history[0].stake_cents, 1200);
    assert!(app.world().resource::<Phone>().animation_left > 0.0);
}

#[test]
fn held_duplicate_stale_busy_paused_and_outside_bet_presses_never_autoplay() {
    let bet = PhoneCommand::Apply(Action::Bet {
        choice: BetChoice::RouletteRed,
        stake_cents: 100,
    });
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    click_command(&mut app, PhoneCommand::SelectBet(BetChoice::RouletteRed));
    click_command(&mut app, bet);
    let settled = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    click_command(&mut app, bet); // Busy presentation rejects a queued second wager.
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    click_command(&mut app, PhoneCommand::Skip);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    for _ in 0..5 {
        let mut phone = app.world_mut().resource_mut::<Phone>();
        phone.pending = Some((PhonePage::Roulette, bet));
        app.update();
    }
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset(MouseButton::Left);
    for scenario in 0..4 {
        opened(&mut app, PhonePage::Roulette);
        {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pointer = if scenario == 0 { None } else { Some(Vec2::ONE) };
            phone.pending = Some((
                if scenario == 1 {
                    PhonePage::Slots
                } else {
                    PhonePage::Roulette
                },
                bet,
            ));
        }
        if scenario == 2 {
            *app.world_mut().resource_mut::<NextState<GameState>>() =
                NextState::Pending(GameState::Menu);
        }
        if scenario == 3 {
            let mut windows = app
                .world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>();
            windows.single_mut(app.world_mut()).unwrap().focused = false;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        *app.world_mut().resource_mut::<NextState<GameState>>() = NextState::Unchanged;
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
    }
}

#[test]
fn shared_mouse_modes_close_once_and_restore_settled_session_without_rng_or_pa_change() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    wager(&mut app, BetChoice::RouletteRed);
    click_command(&mut app, PhoneCommand::Skip);
    click_command(&mut app, PhoneCommand::Page(PhonePage::Betting));
    click_command(&mut app, PhoneCommand::Page(PhonePage::Slots));
    wager(&mut app, BetChoice::Slots);
    click_command(&mut app, PhoneCommand::Skip);
    click_command(&mut app, PhoneCommand::Page(PhonePage::Football));
    wager(&mut app, BetChoice::FootballAway);
    let settled = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    assert_eq!(settled.betting.history.len(), 3);
    assert_eq!(settled.clock.pa_spent, 1);
    *app.world_mut().resource_mut::<Phone>() = Phone::default();
    app.world_mut().resource_mut::<GameplayOverlay>().active = None;
    app.add_systems(PostUpdate, restore_session);
    app.update();
    let phone = app.world().resource::<Phone>();
    assert!(phone.opened);
    assert_eq!(phone.page, PhonePage::Football);
    assert_eq!(phone.animation_left, 0.0);
    assert_eq!(phone.settled_round.as_ref(), settled.betting.history.last());
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    key(&mut app, KeyCode::KeyH);
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .betting
            .session
            .is_none()
    );
    let closed = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    app.update();
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&closed)
    );
    assert_eq!(closed.rng, settled.rng);
    assert_eq!(closed.balance_cents, settled.balance_cents);
}
#[test]
fn visible_mouse_navigation_remains_usable_while_editing_stake() {
    for command in [
        PhoneCommand::Page(PhonePage::Home),
        PhoneCommand::Page(PhonePage::Football),
        PhoneCommand::Close,
    ] {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        click_command(&mut app, PhoneCommand::EditStake);
        key(&mut app, KeyCode::Digit2);
        let before = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        click_command(&mut app, command);
        let phone = app.world().resource::<Phone>();
        assert!(phone.stake_entry.is_none());
        match command {
            PhoneCommand::Page(page) => assert_eq!(phone.page, page),
            _ => assert!(!phone.opened),
        }
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&before)
        );
    }
}
#[test]
fn retry_save_remains_usable_without_discarding_a_stake_draft() {
    let mut app = app();
    opened(&mut app, PhonePage::Slots);
    click_command(&mut app, PhoneCommand::EditStake);
    key(&mut app, KeyCode::Digit2);
    app.world_mut().resource_mut::<CampaignStore>().notice = "old warning".into();
    click_command(&mut app, PhoneCommand::RetrySave);
    assert_eq!(
        app.world().resource::<Phone>().stake_entry.as_deref(),
        Some("2")
    );
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .notice
            .contains("protected")
    );
}
#[test]
fn stop_stays_exposed_and_usable_during_stake_editing_without_settling_again() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    wager(&mut app, BetChoice::RouletteRed);
    let settled = super::super::campaign_save::encode_campaign(
        app.world().resource::<CampaignStore>().campaign().unwrap(),
    )
    .unwrap();
    click_command(&mut app, PhoneCommand::EditStake);
    key(&mut app, KeyCode::Digit2);
    key(&mut app, KeyCode::Digit1);
    let command = {
        let phone = app.world().resource::<Phone>();
        assert_eq!(phone.stake_entry.as_deref(), Some("21"));
        assert!(phone.animation_left > 0.0); // Numeric input edits the draft, including Stop's shortcut number.
        let c = app.world().resource::<CampaignStore>().campaign().unwrap();
        let choices = page_actions(phone, c);
        choices
            .iter()
            .find(|(_, command)| *command == PhoneCommand::Skip)
            .unwrap()
            .1
    };
    click_command(&mut app, command);
    let phone = app.world().resource::<Phone>();
    assert_eq!(phone.animation_left, 0.0);
    assert_eq!(phone.stake_entry.as_deref(), Some("21"));
    assert_eq!(
        super::super::campaign_save::encode_campaign(
            app.world().resource::<CampaignStore>().campaign().unwrap()
        )
        .unwrap(),
        settled
    );
}
#[test]
fn ordered_amount_events_preserve_reverse_digits_repeats_and_backspace_in_one_frame() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    let original = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    // Text received before opening the editor must be discarded.
    amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
    click_command(&mut app, PhoneCommand::EditStake);
    assert_eq!(
        app.world().resource::<Phone>().stake_entry.as_deref(),
        Some("")
    );
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    amount_input(&mut app, KeyCode::Digit8, Some("8"), false);
    app.update();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.insert_resource(State::new(GameState::Menu));
    amount_input(&mut app, KeyCode::Digit7, Some("7"), false);
    app.update();
    app.insert_resource(State::new(GameState::Playing));
    app.update();
    assert_eq!(
        app.world().resource::<Phone>().stake_entry.as_deref(),
        Some("")
    );
    for (key, text, repeat) in [
        (KeyCode::Digit2, Some("2"), false),
        (KeyCode::Digit1, Some("1"), false),
        (KeyCode::Digit1, Some("1"), true),
        (KeyCode::Digit1, Some("1"), true),
        (KeyCode::Backspace, None, false),
        (KeyCode::Digit3, Some("3"), false),
    ] {
        amount_input(&mut app, key, text, repeat);
    }
    app.update();
    assert_eq!(
        app.world().resource::<Phone>().stake_entry.as_deref(),
        Some("2113")
    );
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    // Backspace and confirmation also honor their original order in one frame.
    for _ in 0..2 {
        amount_input(&mut app, KeyCode::Backspace, None, false);
    }
    amount_input(&mut app, KeyCode::Enter, None, false);
    app.update();
    assert_eq!(app.world().resource::<Phone>().stake_cents, 2100);
    assert!(app.world().resource::<Phone>().stake_entry.is_none());
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
}

#[test]
fn contacts_remain_readable_in_blackout_at_zero_pa() {
    let mut c = Campaign::legacy(5);
    for _ in 1..6 {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }
    for _ in 0..6 {
        c.apply(Action::Wait).unwrap();
    }
    c.support.app_blocked_today = true;
    assert!(!c.has_power());
    let original = c.clone();
    let mut app = app();
    app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
    opened(&mut app, PhonePage::Contacts);
    let choices = page_actions(
        &Phone {
            page: PhonePage::Contacts,
            ..default()
        },
        &original,
    );
    assert_eq!(choices.len(), 2);
    key(&mut app, KeyCode::KeyH);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    opened(&mut app, PhonePage::Contacts);
    key(&mut app, KeyCode::Digit1);
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::ContactThread(Contact::Marina)
    );
    key(&mut app, KeyCode::Digit1);
    assert!(app.world().resource::<Phone>().feedback.contains("1 PA"));
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
}

#[test]
fn contacts_dispatch_the_matching_call_and_reject_visit_and_held_commands() {
    let mut c = campaign_after_landlord(5);
    c.apply(Action::Sleep).unwrap();
    c.apply(Action::AcknowledgeReport).unwrap();
    for _ in 0..3 {
        c.apply(Action::Wait).unwrap();
    }
    let mut app = app();
    app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
    opened(&mut app, PhonePage::Contacts);
    click_command(
        &mut app,
        PhoneCommand::Apply(Action::ReceiveVisit(Contact::Rafa)),
    );
    assert_eq!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clock
            .pa_spent,
        3
    );
    key(&mut app, KeyCode::Digit1);
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::ContactThread(Contact::Marina)
    );
    key(&mut app, KeyCode::Digit1);
    let settled = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    assert_eq!(settled.clock.pa_spent, 4);
    assert_eq!(settled.commitments[0].status, CommitmentStatus::Completed);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    for _ in 0..3 {
        app.world_mut().resource_mut::<Phone>().pending = Some((
            PhonePage::ContactThread(Contact::Marina),
            PhoneCommand::Apply(Action::Call(Contact::Marina)),
        ));
        app.update();
    }
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .notice
            .contains("session")
    );
}

#[test]
fn home_reminder_combines_todays_calls_and_bill_counts_during_a_blackout() {
    let mut c = campaign_after_landlord(5);
    for _ in 1..4 {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }
    let reminder = home_reminder(&c);
    assert_eq!(
        reminder,
        "Rafa calls · night\n1 bill due today\n2 overdue bills\nPower off · electricity bill on the table"
    );
    assert!(rendered_copy(PhonePage::Home, &c).contains(&reminder));

    let mut older = Campaign::previous_economy(5);
    for _ in 1..4 {
        older.apply(Action::Sleep).unwrap();
        older.apply(Action::AcknowledgeReport).unwrap();
    }
    assert_eq!(home_reminder(&older), "Rafa visits · afternoon\n2 bills due today");
    older.apply(Action::PayBill(BillKind::Electricity)).unwrap();
    assert_eq!(home_reminder(&older), "Rafa visits · afternoon\n1 bill due today");
}

#[test]
fn portuguese_phone_copy_follows_the_campaign_on_all_apps() {
    use crate::localization::Language::Portuguese;

    let mut campaign = campaign_after_landlord(5);
    campaign.apply(Action::Bet { choice: BetChoice::Slots, stake_cents: 100 }).unwrap();
    for _ in 1..campaign.last_day() {
        campaign.apply(Action::Sleep).unwrap();
        campaign.apply(Action::AcknowledgeReport).unwrap();
    }
    for (page, expected) in [
        (PhonePage::Home, &["Rafa liga · noite", "1 conta vence hoje", "2 contas vencidas", "Energia cortada"][..]),
        (PhonePage::History, &["Histórico de todos os dias", "Gasto: R$1,00", "Apostas"]),
        (PhonePage::Contacts, &["Mensagens", "Perdemos o horário combinado.", "Dia 4 · Noite"]),
        (PhonePage::ContactThread(Contact::Rafa), &["ei mano você tá bem?", "Ligar para Rafa · 1 PA"]),
        (PhonePage::Support, &["Sessão de terapia", "Limite diário de apostas"]),
        (PhonePage::DailyCap, &["Aplicar limite de R$0,00", "Os prêmios não repõem o limite"]),
        (PhonePage::Betting, &["Cassino", "Caça-níqueis", "Regras"]),
        (PhonePage::BettingHistory, &["Minhas apostas", "Caça-níqueis · Dia 1 / Manhã", "R$1,00"]),
        (PhonePage::Slots, &["Retorno R$", "Girar R$1,00"]),
        (PhonePage::Roulette, &["Vermelho · 1,8×", "Preto · 1,8×"]),
        (PhonePage::Football, &["Mandante", "Visitante", "Empate"]),
        (PhonePage::ConfirmSale(HouseholdItem::Fridge), &["Revisar venda definitiva", "Geladeira", "Receber R$120,00"]),
    ] {
        let copy = rendered_copy_in_language(page, &campaign, Portuguese);
        for text in expected {
            assert!(copy.contains(text), "{page:?}: {text:?} missing from {copy}");
        }
    }
}

#[test]
fn contact_threads_show_each_short_message_and_keep_the_call_action() {
    let mut c = campaign_after_landlord(5);
    assert_eq!(home_reminder(&c), "Marina: day 2 afternoon");
    c.apply(Action::Bet {
        choice: BetChoice::Slots,
        stake_cents: 100,
    })
    .unwrap();
    for _ in 1..3 {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }
    for (contact, message) in [
        (Contact::Marina, "I'm here if you need anything. You know that, right?"),
        (Contact::Rafa, "Hey man, you okay?"),
    ] {
        let copy = rendered_copy(PhonePage::ContactThread(contact), &c);
        assert!(copy.contains(message));
        assert!(copy.contains(&format!("Call {} · 1 PA", contact.label())));
        for detail in ["R$", "unpaid bills", "We missed our plan", "Relationships +8", "Day 4"] {
            assert!(!copy.contains(detail), "{detail}");
        }
    }
}

#[test]
fn therapy_session_costs_four_pa_and_takes_thirty_urge() {
    let mut app = app();
    let mut campaign = campaign_after_landlord(9);
    campaign.attributes.urge = 48;
    app.insert_resource(CampaignStore::for_test(Some(campaign), SnapshotState::Protected));
    opened(&mut app, PhonePage::Support);
    let therapy = PhoneCommand::Apply(Action::TherapySession);
    click_command(&mut app, therapy);
    let after = app.world().resource::<CampaignStore>().campaign().unwrap().clone();
    assert_eq!((after.clock.pa_spent, after.attributes.urge), (4, 18));
    click_command(&mut app, therapy);
    assert_eq!(app.world().resource::<CampaignStore>().campaign(), Some(&after));
    assert!(app.world().resource::<Phone>().feedback.contains("4 PA"));
}

#[test]
fn missed_call_waits_for_the_bet_reveal_and_resumes_the_phone_after_confirmation() {
    let mut app = app();
    let mut campaign = campaign_after_landlord(9);
    campaign.apply(Action::Sleep).unwrap();
    campaign.apply(Action::AcknowledgeReport).unwrap();
    for _ in 0..3 {
        campaign.apply(Action::Wait).unwrap();
    }
    campaign.attributes.urge = 80;
    app.insert_resource(CampaignStore::for_test(Some(campaign), SnapshotState::Protected));
    opened(&mut app, PhonePage::Slots);
    wager(&mut app, BetChoice::Slots);
    let mut text = app.world_mut().query::<&Text>();
    assert!(!text.iter(app.world()).any(|text| text.0.contains("missed my call")));
    click_command(&mut app, PhoneCommand::Skip);
    assert_eq!(
        app.world().resource::<GameplayOverlay>().active,
        Some(OverlayKind::MissedCall { contact: Contact::Marina, resume_phone: true })
    );
    assert!(text.iter(app.world()).any(|text| text.0.contains("missed my call with Marina")));
    key(&mut app, KeyCode::Tab);
    assert!(app.world().resource::<Phone>().opened);
    assert!(app.world().resource::<GameplayOverlay>().in_dialogue());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<GameplayOverlay>().active, Some(OverlayKind::Phone));
    assert!(!text.iter(app.world()).any(|text| text.0.contains("missed my call")));
    let store = app.world().resource::<CampaignStore>();
    let campaign = store.campaign().unwrap();
    assert_eq!(campaign.attributes.relationships, 40);
    assert_eq!(campaign.betting.session.as_ref().unwrap().paid_rounds_remaining, 2);
}

#[test]
fn cap_amount_uses_ordered_events_and_requires_a_separate_explicit_apply() {
    let mut app = app();
    opened(&mut app, PhonePage::DailyCap);
    let original = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
    click_command(&mut app, PhoneCommand::EditCap);
    assert_eq!(
        app.world().resource::<Phone>().cap_entry.as_deref(),
        Some("")
    );
    for (key, text, repeat) in [
        (KeyCode::Digit2, Some("2"), false),
        (KeyCode::Digit1, Some("1"), false),
        (KeyCode::Digit1, Some("1"), true),
        (KeyCode::Backspace, None, false),
        (KeyCode::Enter, None, false),
    ] {
        amount_input(&mut app, key, text, repeat);
    }
    app.update();
    assert_eq!(app.world().resource::<Phone>().cap_cents, 2100);
    assert!(app.world().resource::<Phone>().cap_entry.is_none());
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    click_command(&mut app, PhoneCommand::Apply(Action::SetDailyCap(Some(0))));
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    click_command(
        &mut app,
        PhoneCommand::Apply(Action::SetDailyCap(Some(2100))),
    );
    assert_eq!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .support
            .daily_cap_cents,
        Some(2100)
    );
    assert!(
        app.world()
            .resource::<Phone>()
            .feedback
            .contains("Limit updated")
    );
    assert!(!app.world().resource::<Phone>().feedback.contains("saved"));
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .notice
            .contains("protected")
    );
    click_command(&mut app, PhoneCommand::EditCap);
    key(&mut app, KeyCode::Digit0);
    click_command(&mut app, PhoneCommand::ConfirmCap);
    assert_eq!(app.world().resource::<Phone>().cap_cents, 0);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Support);
}

#[test]
fn cap_editor_drains_inactive_text_and_keeps_navigation_and_retry_usable() {
    let mut app = app();
    opened(&mut app, PhonePage::DailyCap);
    click_command(&mut app, PhoneCommand::EditCap);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
    app.update();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.insert_resource(State::new(GameState::Menu));
    amount_input(&mut app, KeyCode::Digit8, Some("8"), false);
    app.update();
    app.insert_resource(State::new(GameState::Playing));
    app.update();
    assert_eq!(
        app.world().resource::<Phone>().cap_entry.as_deref(),
        Some("")
    );
    key(&mut app, KeyCode::Digit2);
    key(&mut app, KeyCode::Digit1);
    let original = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    click_command(&mut app, PhoneCommand::RetrySave);
    assert_eq!(
        app.world().resource::<Phone>().cap_entry.as_deref(),
        Some("21")
    );
    click_command(
        &mut app,
        PhoneCommand::Apply(Action::SetDailyCap(Some(2100))),
    );
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
    click_command(&mut app, PhoneCommand::Page(PhonePage::Home));
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Home);
    assert!(app.world().resource::<Phone>().cap_entry.is_none());
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&original)
    );
}

#[test]
fn care_dispatch_rejects_held_stale_unfocused_and_zero_pa_actions() {
    let mut app = app();
    opened(&mut app, PhonePage::Support);
    let command = PhoneCommand::Apply(Action::CareConversation);
    click_command(&mut app, command);
    let after = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    assert_eq!((after.clock.pa_spent, after.support.care_actions), (1, 1));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    for _ in 0..3 {
        app.world_mut().resource_mut::<Phone>().pending = Some((PhonePage::Support, command));
        app.update();
    }
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&after)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset(MouseButton::Left);
    for scenario in 0..3 {
        opened(&mut app, PhonePage::Support);
        {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pointer = if scenario == 0 { None } else { Some(Vec2::ONE) };
            phone.pending = Some((
                if scenario == 1 {
                    PhonePage::Contacts
                } else {
                    PhonePage::Support
                },
                command,
            ));
        }
        if scenario == 2 {
            let mut query = app
                .world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>();
            query.single_mut(app.world_mut()).unwrap().focused = false;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        query.single_mut(app.world_mut()).unwrap().focused = true;
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&after)
        );
    }
    let mut exhausted = after.clone();
    while exhausted.clock.remaining_pa() > 0 {
        exhausted.apply(Action::Wait).unwrap();
    }
    app.insert_resource(CampaignStore::for_test(
        Some(exhausted.clone()),
        SnapshotState::Protected,
    ));
    click_command(&mut app, command);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&exhausted)
    );
    assert!(app.world().resource::<Phone>().feedback.contains("1 PA"));
}

#[test]
fn support_keeps_care_and_betting_limits_available_during_blackout() {
    let mut c = Campaign::legacy(5);
    for _ in 1..6 {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }
    for _ in 0..6 {
        c.apply(Action::Wait).unwrap();
    }
    c.apply(Action::BlockBetting).unwrap();
    assert!(!c.has_power());
    let mut app = app();
    app.insert_resource(CampaignStore::for_test(
        Some(c.clone()),
        SnapshotState::Protected,
    ));
    opened(&mut app, PhonePage::Support);
    let choices = page_actions(
        &Phone {
            page: PhonePage::Support,
            ..default()
        },
        &c,
    );
    assert_eq!(
        choices.into_iter().map(|(_, command)| command).collect::<Vec<_>>(),
        [
            PhoneCommand::Apply(Action::CareConversation),
            PhoneCommand::Apply(Action::FinancialPlanning),
            PhoneCommand::Apply(Action::TherapySession),
            PhoneCommand::Page(PhonePage::DailyCap),
        ]
    );
    click_command(&mut app, PhoneCommand::Page(PhonePage::DailyCap));
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::DailyCap);
    assert_eq!(app.world().resource::<CampaignStore>().campaign(), Some(&c));
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.world().resource::<Phone>().page, PhonePage::Support);
}

#[test]
fn entering_support_ends_session_and_block_keeps_the_committed_result() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    wager(&mut app, BetChoice::RouletteRed);
    let before = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    let result = app.world().resource::<Phone>().settled_round.clone();
    click_command(&mut app, PhoneCommand::Page(PhonePage::Home));
    click_command(&mut app, PhoneCommand::Page(PhonePage::Support));
    assert!(
        app.world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .betting
            .session
            .is_none()
    );
    click_command(&mut app, PhoneCommand::Apply(Action::BlockBetting));
    let after = app.world().resource::<CampaignStore>().campaign().unwrap();
    assert!(after.support.app_blocked_today);
    assert_eq!(after.betting.history, before.betting.history);
    assert_eq!(after.rng, before.rng);
    assert_eq!(after.balance_cents, before.balance_cents);
    assert_eq!(app.world().resource::<Phone>().settled_round, result);
}

#[test]
fn selection_rules_and_my_bets_preserve_session_until_support() {
    let mut app = app();
    opened(&mut app, PhonePage::Roulette);
    let initial = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    click_command(&mut app, PhoneCommand::SelectBet(BetChoice::RouletteBlack));
    assert_eq!(
        app.world().resource::<Phone>().selected_bet,
        Some(BetChoice::RouletteBlack)
    );
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&initial)
    );
    wager(&mut app, BetChoice::RouletteBlack);
    click_command(&mut app, PhoneCommand::Skip);
    let settled = app
        .world()
        .resource::<CampaignStore>()
        .campaign()
        .unwrap()
        .clone();
    assert!(settled.betting.session.is_some());
    click_command(&mut app, PhoneCommand::ToggleBetInfo);
    assert!(app.world().resource::<Phone>().bet_info_open);
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    click_command(&mut app, PhoneCommand::Page(PhonePage::BettingHistory));
    assert_eq!(
        app.world().resource::<Phone>().page,
        PhonePage::BettingHistory
    );
    assert_eq!(
        app.world().resource::<CampaignStore>().campaign(),
        Some(&settled)
    );
    click_command(&mut app, PhoneCommand::Page(PhonePage::Support));
    let after = app.world().resource::<CampaignStore>().campaign().unwrap();
    assert!(after.betting.session.is_none());
    assert_eq!(after.balance_cents, settled.balance_cents);
    assert_eq!(after.clock, settled.clock);
    assert_eq!(after.betting.history, settled.betting.history);
    assert_eq!(after.rng, settled.rng);
}

#[test]
fn animation_progress_retains_phone_entities_and_scroll() {
    let mut app = app();
    app.insert_resource(PhoneAssets {
        camera: Entity::PLACEHOLDER,
        target: Handle::default(),
        font: Handle::default(),
    })
    .add_systems(
        Update,
        (present, betting::animate).chain().after(end_session),
    );
    opened(&mut app, PhonePage::Slots);
    wager(&mut app, BetChoice::Slots);
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<PhoneUi>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .query_filtered::<&mut ScrollPosition, With<PhoneScroll>>()
        .single_mut(app.world_mut())
        .unwrap()
        .0
        .y = 24.;
    for left in [1.5, 0.9, 0.3] {
        app.world_mut().resource_mut::<Phone>().animation_left = left;
        app.update();
        assert!(app.world().get_entity(root).is_ok());
        assert_eq!(
            app.world_mut()
                .query_filtered::<&ScrollPosition, With<PhoneScroll>>()
                .single(app.world())
                .unwrap()
                .0
                .y,
            24.
        );
    }
}
#[derive(Resource, Default)]
struct HeardSounds(Vec<Sound>);

fn audio_app() -> App {
    let mut campaign = campaign_after_landlord(17);
    campaign
        .apply(Action::Bet {
            choice: BetChoice::Slots,
            stake_cents: 100,
        })
        .unwrap();
    let mut app = App::new();
    app.init_resource::<Time<Real>>()
        .init_resource::<HeardSounds>()
        .insert_resource(State::new(GameState::Playing))
        .insert_resource(Phone {
            opened: true,
            page: PhonePage::Slots,
            animation_left: 0.1,
            settled_round: campaign.betting.history.last().cloned(),
            ..default()
        })
        .add_observer(|sound: On<Sound>, mut heard: ResMut<HeardSounds>| {
            heard.0.push(*sound.event());
        })
        .add_systems(Update, tick_game);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_millis(200));
    app
}

#[test]
fn audio_reveal_plays_once_and_waits_for_focus() {
    let mut app = audio_app();
    let expected = if app
        .world()
        .resource::<Phone>()
        .settled_round
        .as_ref()
        .unwrap()
        .net_cents
        > 0
    {
        Sound::Win
    } else {
        Sound::Loss
    };
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(app.world().resource::<HeardSounds>().0.is_empty());
    assert_eq!(app.world().resource::<Phone>().animation_left, 0.1);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    app.update();
    assert_eq!(app.world().resource::<HeardSounds>().0, vec![expected]);
}

#[test]
fn audio_skip_reveals_once_and_restored_results_stay_silent() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = audio_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands, mut phone: ResMut<Phone>| {
            reveal_sound(&phone, &mut commands);
            phone.animation_left = 0.0;
            reveal_sound(&phone, &mut commands);
        })
        .unwrap();
    app.update();
    app.update();
    assert_eq!(app.world().resource::<HeardSounds>().0.len(), 1);
    app.world_mut().resource_mut::<HeardSounds>().0.clear();
    app.update();
    assert!(app.world().resource::<HeardSounds>().0.is_empty());
}
