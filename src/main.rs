use std::path::PathBuf;

use baguette::*;
use baguette::app::ui;
use egui_plot as plot;
use std::collections::VecDeque;
use indexmap::IndexMap;

fn main()
{
    baguette::new()
        .set_title("baguette tilemap editor")
        .add_loop::<Application>()
        .run()
}

type Tiles = IndexMap<TilePos,ui::Rect>;

struct Application
{
    path: PathBuf,
    asset_preview_scale: f32,
    selected_tile: Option<(usize, ui::Rect)>,

    /// drag state to check if we need to draw
    dragging: Option<Tiles>,

    /// the tiles we actually render each frame
    tiles: Tiles,
    /// cronology of the tiles after each modification
    tiles_history: TilesHistory
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TilePos
{
    x: i32, y: i32
}

struct TilesHistory(VecDeque<Tiles>, u16);

impl TilesHistory
{
    fn new() -> Self
    {
        Self(Default::default(), 5)
    }

    //fn is_empty(&self) -> bool
    //{
    //    self.0.is_empty()
    //}

    /// add an undo operation
    fn add(&mut self, tiles: IndexMap<TilePos, ui::Rect>)
    {
        if self.0.len() >= self.1 as usize
        {
            self.0.pop_front();
        }

        self.0.push_back(tiles)
    }

    fn undo(&mut self) -> Option<IndexMap<TilePos, ui::Rect>>
    {
        self.0.pop_back()
    }
}

impl app::State for Application
{
    fn new(app: &mut app::App) -> Self where Self: Sized
    {
        egui_extras::install_image_loaders(app.ui().context());
        
        Self
        {
            path: PathBuf::new(),
            asset_preview_scale: 1.,
            selected_tile: None,
            tiles: Tiles::default(),
            tiles_history: TilesHistory::new(),
            dragging: None
        }
    }

    fn update(&mut self, app: &mut app::App, _: &app::StateEvent)
    {
        self.top_panel(app);
        self.bottom_panel(app);
        
        self.editor_grid(app);

        if app.input.get_key_down
        (
            input::KeyCode::KeyZ) && app.input.get_key_holding(input::KeyCode::ControlLeft
        )
        {
            let Some(undo_tiles) = self.tiles_history.undo() else 
            {
                return
            };

            for (pos, uv) in undo_tiles
            {
                if uv == ui::Rect::NOTHING { self.tiles.remove(&pos); }
                else { self.tiles.insert(pos, uv); }
            }
        }

    }
}

impl Application
{
    fn top_panel(&mut self, app: &mut app::App)
    {
        let frame = ui::Frame
        {
            inner_margin: ui::Margin::same(2.),
            fill: ui::Color32::from_gray(60),
            ..Default::default()
        };

        let contents = |ui: &mut ui::egui::Ui|
        {
            ui.horizontal_centered
            (
                |ui|
                {
                    let file = ui.button
                    (
                        ui::RichText::new("file")
                        .size(15.)
                        .color(ui::Color32::from_gray(240))
                    );

                    if file.clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("", &["png"])
                            .set_file_name("choose a spritesheet")
                            .pick_file()
                        {
                            self.path = path
                        }
                    }
                    
                    let reset = ui.button
                    (
                        ui::RichText::new("reset")
                        .size(15.)
                        .color(ui::Color32::from_gray(240))
                    );
 
                    if reset.clicked()
                    {
                        let tiles = self.tiles.clone();    

                        for (.., uv) in self.tiles.iter_mut()
                        {
                            *uv = ui::Rect::NOTHING
                        }

                        self.tiles_history.add(tiles);
                    }
                }
            )
        };

        ui::TopBottomPanel::top("path")
            .frame(frame)
            .show(app.ui().context(), contents);
    }

    fn bottom_panel(&mut self, app: &mut app::App)
    {
        ui::TopBottomPanel::bottom("assets")
        .frame(ui::Frame
        {
            inner_margin: ui::Margin::symmetric(1., 5.),
            fill: ui::Color32::from_gray(35),
            ..Default::default()
        })
        .show(app.ui().context(), |ui|
        {
            if !self.path.is_file()
            {
                return
            }
    
            ui.label(self.path.to_string_lossy());

            ui.separator();

            ui.add
            (
                ui::Slider::new(&mut self.asset_preview_scale, 0.3..=3.)
                    .handle_shape(ui::style::HandleShape::Rect
                    {
                        aspect_ratio: 0.75
                    })
                    .trailing_fill(true)
                    .show_value(false)
            );
            let scale = 100. * self.asset_preview_scale;

            let collapsable_contents = |ui: &mut ui::egui::Ui|
            {
                let style = ui.style_mut();

                style.spacing.button_padding = (0.1, 0.1).into();
                style.spacing.item_spacing = (2.5, 2.5).into();

                style.visuals.widgets.hovered.bg_stroke = ui::Stroke::new(scale / 100., ui::Color32::LIGHT_GRAY);
                style.visuals.selection.stroke = ui::Stroke::new(scale / 25., ui::Color32::LIGHT_GRAY);

                for (idx, image)
                    in load_images("file://D:/dev/Rust/tilemap test/test_sheet.png", 2,2)
                        .enumerate()
                {
                    let selected = self.selected_tile
                        .is_some_and(|(sel_idx, ..)| idx == sel_idx);
                    
                    let uv = image.image_options().uv;

                    let tile_display = ui.add_sized
                    (
                        (scale,scale),
                        ui::Button::image(image)
                            .fill(ui::Color32::TRANSPARENT)
                            .selected(selected)

                    );

                    if tile_display.clicked()
                    {
                        self.selected_tile = Some((idx,uv))
                    }
                }
            };

            let header_text = ui::RichText::new("tiles")
                .size(15.)
                .monospace()
                .color(ui::Color32::from_rgb(100, 100, 100));

            ui::CollapsingHeader::new(header_text)
                .default_open(true)
                .show(ui, |ui| ui.horizontal_wrapped(collapsable_contents));
        });
    }

    fn editor_grid(&mut self, app: &mut app::App)
    {
        let plot_contents = |ui: &mut plot::PlotUi|
        {
            ui.vline(plot::VLine::new(0.).color(ui::Color32::GRAY));
            ui.hline(plot::HLine::new(0.).color(ui::Color32::GRAY));

            // use the middle click instead of left click
            if ui.response().dragged_by(ui::PointerButton::Middle)
            {
                ui.ctx().set_cursor_icon(ui::CursorIcon::Grabbing);
                ui.translate_bounds(-ui.pointer_coordinate_drag_delta())
            }

            if let Some(screen_pos) = ui.response().hover_pos()
            {
                let mut pos = ui.plot_from_screen(screen_pos);

                let floor_pos = plot::PlotPoint { x: pos.x.floor(), y: pos.y.floor() };
    
                pos.x = floor_pos.x.floor() + 0.5;
                pos.y = floor_pos.y.floor() + 0.5;
    
                let response = ui.response();
                
                // this means we have no tile selected to draw,
                // meaning we don't need to draw anything the on tiles
                // so we just return
                let Some((.., selected_uv)) = self.selected_tile else
                {
                    return draw_tiles(&mut self.tiles, ui)
                };
    
                if response.drag_started_by(ui::PointerButton::Primary)
                {
                    self.dragging = Some(indexmap::IndexMap::with_capacity(8))
                }
                else if response.drag_released_by(ui::PointerButton::Primary)
                {
                    self.tiles_history.add(self.dragging.take().unwrap())
                }

                if let Some(ref mut current_edit_tiles) = self.dragging
                {
                    let pos = TilePos
                    {
                        x: floor_pos.x as i32,
                        y: floor_pos.y as i32
                    };

                    if current_edit_tiles.get(&pos).is_none()
                    {
                        match self.tiles.insert(pos, selected_uv)
                        {
                            Some(old_uv) =>
                            {
                                current_edit_tiles.insert(pos, old_uv);
                            }
                            None =>
                            {
                                current_edit_tiles.insert(pos, ui::Rect::NOTHING);
                            }
                        }
                    }
                }

                ui.image
                (
                    plot::PlotImage::new(ui::TextureId::Managed(1),
                    pos, (1., 1.)
                )
                    .highlight(true)
                    .uv(selected_uv));
            }

            draw_tiles(&mut self.tiles, ui);
            
            fn draw_tiles (tiles: &mut Tiles, ui: &mut plot::PlotUi)
            {
                for (TilePos { x, y }, uv) in tiles
                {
                    ui.image(plot::PlotImage::new
                    (
                        ui::TextureId::Managed(1),
                        plot::PlotPoint { x: *x as f64 + 0.5, y: *y as f64 + 0.5 },
                        (1., 1.)
                    )
                    .uv(*uv))
                }
            }
        };

        let panel_contents = |ui: &mut ui::egui::Ui|
        {
            plot::Plot::new("tilemap display")
                .data_aspect(1.)

                .x_grid_spacer(plot::log_grid_spacer(1))
                .y_grid_spacer(plot::log_grid_spacer(1))

                .allow_double_click_reset(false)
                
                .allow_drag(false)
                .allow_boxed_zoom(false)
                .show_background(false)
                
                .show(ui, plot_contents)
        };

        ui::CentralPanel::default()
            .frame(ui::Frame
            {
                inner_margin: ui::Margin::symmetric(1., 5.),
                fill: ui::Color32::from_gray(45),
                ..Default::default()
            })
            .show(app.ui().context(), panel_contents);
    }
}

fn load_images<'a>
(
    image: impl Into<ui::ImageSource<'a>>,
    rows: usize,
    columns: usize
) -> impl Iterator<Item = ui::Image<'a>>
{
    let mut items = Vec::with_capacity(rows * columns);
    
    let image = ui::Image::new(image);

    for column in 0..columns
    {
        let vmax = 0. + (1. / columns as f32) * (column + 1) as f32;
        let vmin = 0. + (1. / columns as f32) * column as f32;

        for row in 0..rows
        {
            let umax = 0. + (1. / rows as f32) * (row + 1) as f32;
            let umin = 0. + (1. / rows as f32) * row as f32;

            items.push
            (
                image
                    .clone()
                    .texture_options(ui::TextureOptions::NEAREST)
                    .uv([ui::pos2(umin, vmin), ui::pos2(umax , vmax)])
            )
        };
    }

    items.into_iter()
}
