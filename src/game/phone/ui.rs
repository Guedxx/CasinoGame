use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tile {
    Roulette,
    Slots,
    Football,
    Seven,
    Bar,
    Bell,
    Marina,
    Rafa,
    Tv,
    Fridge,
    Stove,
    Cookware,
    Couch,
    Books,
    Pipoca,
}

#[derive(Resource)]
pub(super) struct Art {
    betting: Handle<Image>,
    apps: Handle<Image>,
    pub wheel: Handle<Image>,
    pub regular: Handle<Font>,
}

impl FromWorld for Art {
    fn from_world(world: &mut World) -> Self {
        let wheel = world
            .resource_mut::<Assets<Image>>()
            .add(betting::wheel_image());
        let assets = world.resource::<AssetServer>();
        let bitmap = |path: &'static str| {
            assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::nearest();
                })
                .load(path)
        };
        Self {
            betting: bitmap("ui/phone-betting.png"),
            apps: bitmap("ui/phone-apps.png"),
            wheel,
            regular: assets.load("fonts/LiberationSans-Regular.ttf"),
        }
    }
}

impl Art {
    pub fn image(&self, tile: Tile) -> ImageNode {
        use Tile::*;
        let (betting, x, y, size) = match tile {
            Roulette => (true, 0., 0., 64.),
            Slots => (true, 64., 0., 64.),
            Football => (true, 0., 64., 64.),
            Seven => (true, 64., 64., 32.),
            Bar => (true, 96., 64., 32.),
            Bell => (true, 64., 96., 32.),
            Marina => (false, 128., 0., 32.),
            Rafa => (false, 160., 0., 32.),
            Tv => (false, 0., 64., 48.),
            Fridge => (false, 48., 64., 48.),
            Stove => (false, 96., 64., 48.),
            Cookware => (false, 144., 64., 48.),
            Couch => (false, 0., 112., 48.),
            Books => (false, 48., 112., 48.),
            Pipoca => (false, 96., 112., 48.),
        };
        ImageNode::new(if betting { &self.betting } else { &self.apps }.clone()).with_rect(
            Rect::from_corners(Vec2::new(x, y), Vec2::new(x + size, y + size)),
        )
    }
}

pub(super) fn picture(
    parent: &mut ChildSpawnerCommands,
    art: &Art,
    tile: Tile,
    width: f32,
    height: f32,
) {
    parent.spawn((
        art.image(tile),
        Node {
            width: px(width),
            height: px(height),
            flex_shrink: 0.,
            ..default()
        },
        Pickable::IGNORE,
    ));
}

pub(super) fn row() -> Node {
    Node {
        width: percent(100),
        column_gap: px(8),
        align_items: AlignItems::Center,
        flex_shrink: 0.,
        ..default()
    }
}

pub(super) fn column() -> Node {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(8),
        flex_shrink: 0.,
        ..default()
    }
}

#[derive(Component)]
struct ButtonFill(Color);

pub(super) fn action(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    page: PhonePage,
    label: &str,
    command: PhoneCommand,
    fill: Color,
    ink: Color,
) -> Entity {
    parent
        .spawn((
            Name::new(label.to_owned()),
            Button,
            PhoneButton(page, command),
            ButtonFill(fill),
            BackgroundColor(fill),
            Node {
                width: percent(100),
                min_height: px(44),
                flex_shrink: 0.,
                padding: UiRect::axes(px(10), px(8)),
                border_radius: BorderRadius::all(px(10)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .observe(queue_press)
        .observe(
            |over: On<Pointer<Over>>, mut buttons: Query<(&ButtonFill, &mut BackgroundColor)>| {
                if let Ok((fill, mut color)) = buttons.get_mut(over.entity) {
                    let base = fill.0.to_srgba();
                    color.0 = Color::srgba(
                        (base.red + 0.08).min(1.),
                        (base.green + 0.08).min(1.),
                        (base.blue + 0.08).min(1.),
                        base.alpha,
                    );
                }
            },
        )
        .observe(
            |out: On<Pointer<Out>>, mut buttons: Query<(&ButtonFill, &mut BackgroundColor)>| {
                if let Ok((fill, mut color)) = buttons.get_mut(out.entity) {
                    color.0 = fill.0;
                }
            },
        )
        .with_children(|child| text(child, font, label, 20., ink))
        .id()
}
