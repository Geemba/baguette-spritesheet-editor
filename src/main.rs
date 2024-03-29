use std::{io::{Read, Write}, path::PathBuf,};

use baguette::{*, app::ui};

use egui_plot as plot;
use serde::{Deserialize, Serialize};

use indexmap::IndexMap;

fn main()
{
    baguette::new()
        .set_title("baguette tilemap editor")
        .add_loop::<Application>()
        .run()
}

#[derive(Clone)]
#[derive(Serialize,Deserialize)]
struct SpriteSheet
{
    path: PathBuf,
    rows: usize,
    columns: usize
}

struct Application
{
    /// the path we loaded the spritesheet from 
    sprite_sheet: Option<SpriteSheet>,
    workspace_path: Option<PathBuf>,
    asset_preview_scale: f32,
    selected_tile: Option<(usize, ui::Rect)>,

    /// drag state to check if we need to draw
    dragging: Option<Tiles>,

    /// the tiles we will actually draw
    tiles: Tiles,

    undos: TilesHistory,
    redos: TilesHistory
}

impl app::State for Application
{
    fn new(app: &mut app::App) -> Self where Self: Sized
    {
        egui_extras::install_image_loaders(app.ui().context());
        
        Self
        {
            sprite_sheet: None,
            asset_preview_scale: 1.,
            selected_tile: None,

            tiles: Tiles::default(),
            undos: TilesHistory::new(),
            redos: TilesHistory::new(),

            dragging: None,
            workspace_path: None,
        }
    }

    fn update(&mut self, app: &mut app::App, _: &app::StateEvent)
    {
        self.top_panel(app);
        self.bottom_panel(app);
        
        self.editor_grid(app);

        self.check_input(app);
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
            let text_style = |text| ui::RichText::new(text)
                .size(15.)
                .color(ui::Color32::from_gray(200));

            ui.horizontal_centered
            (
                |ui|
                {
                    ui.menu_button(text_style("file"), |ui| 
                    {
                        // open a new empty tilemap
                        let new = ui.button
                        (
                            text_style("new")
                        );

                        if new.clicked()
                        {
                            self.select_spritesheet()
                        }

                        // load a tilemap workspace
                        let load = ui.button
                        (
                            text_style("open saved")
                        );

                        if load.clicked()
                        {
                            let _ = self.load_workspace();
                        }

                        // reset button
                        let reset = ui.button(text_style("clear"));
                    
                        if reset.clicked() && !self.tiles.is_empty()
                        {
                            let tiles = self.tiles.clone();
                            self.tiles.clear();
                        
                            self.undos.add(tiles);
                        }
                    })
                }
            )
        };

        ui::TopBottomPanel::top("path")
            .frame(frame)
            .show(app.ui().context(), contents);
    }

    fn select_spritesheet(&mut self)
    {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("", &["png"])
            .set_file_name("choose a spritesheet")
            .pick_file()
            {
                self.sprite_sheet = Some(SpriteSheet { path, rows: 1, columns: 1 });
                self.workspace_path = None
            }
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
            let Some(SpriteSheet { ref path, ref mut rows, ref mut columns }) = self.sprite_sheet else
            {
                return
            };
    
            ui.label(path.to_string_lossy());

            ui.separator();
            
            let scale = 100. * self.asset_preview_scale;

            let collapsable_contents = |ui: &mut ui::egui::Ui|
            {
                ui.group(|ui| ui.vertical(|ui|
                {
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

                        ui.horizontal
                        (
                            |ui|
                            {
                                ui.label(ui::RichText::new("row").monospace());
                                ui.add(ui::DragValue::new(rows));
                            }
                        );
                        
                        ui.horizontal
                        (
                            |ui|
                            {
                                ui.label(ui::RichText::new("columns").monospace());
                                ui.add(ui::DragValue::new(columns));
                            }
                        );
                }));

                let style = ui.style_mut();

                style.spacing.button_padding = (0.1, 0.1).into();
                style.spacing.item_spacing = (2.5, 2.5).into();

                style.visuals.widgets.hovered.bg_stroke = ui::Stroke::new(2.5, ui::Color32::LIGHT_GRAY);
                style.visuals.selection.stroke = ui::Stroke::new(5., ui::Color32::LIGHT_GRAY);

                let uri = "file://".to_owned() + path
                    .to_str()
                    .expect
                    (
                        "received invalid UTF-8, why not just use ostr as source anyway?"
                    );

                for (idx, image) in load_images(uri, *rows, *columns).enumerate()
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
                .color(ui::Color32::from_gray(100));

            

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
                    return
                };
    
                if response.drag_started_by(ui::PointerButton::Primary)
                {
                    self.redos.clear();
                    self.dragging = Some(indexmap::IndexMap::with_capacity(8))
                }
                else if response.drag_released_by(ui::PointerButton::Primary)
                {
                    self.undos.add(self.dragging.take().unwrap())
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

    fn check_input(&mut self, app: &mut app::App)
    {
        if app.input.get_key_down(input::KeyCode::KeyZ)
            && app.input.get_key_holding
            (
                input::KeyCode::ControlLeft
            )
            && !app.input.get_key_holding
            (
                input::KeyCode::ShiftLeft
            )
        {
            let Some(undo_tiles) = self.undos.pop() else 
            {
                return
            };

            // here we will gather the tiles we are replacing with the undo tiles,
            // so that we can use them as redo operation later
            let mut redo_tiles = IndexMap::with_capacity(undo_tiles.len());

            for (pos, uv) in undo_tiles
            {
                if uv == ui::Rect::NOTHING
                {
                    match self.tiles.remove(&pos)
                    {
                        Some(old_uv) => redo_tiles.insert(pos, old_uv),
                        None => redo_tiles.insert(pos, ui::Rect::NOTHING)
                    };
                }
                else
                {
                    match self.tiles.insert(pos, uv)
                    {
                        Some(old_uv) => redo_tiles.insert(pos, old_uv),
                        None => redo_tiles.insert(pos, ui::Rect::NOTHING)
                    };
                }
            }

            self.redos.add(redo_tiles)
        }
        
        if app.input.get_key_down(input::KeyCode::KeyZ)
            && app.input.get_key_holding
            (
                input::KeyCode::ControlLeft
            )
            && app.input.get_key_holding
            (
                input::KeyCode::ShiftLeft
            )
        {
            let Some(redo_tiles) = self.redos.pop() else
            {
                return
            };

            // here we will gather the tiles we are replacing with the redo tiles,
            // so that we can use them as undo operation later
            let mut undo_tiles = IndexMap::with_capacity(redo_tiles.len());

            for (pos, uv) in redo_tiles
            {
                if uv == ui::Rect::NOTHING
                {
                    match self.tiles.remove(&pos)
                    {
                        Some(old_uv) => undo_tiles.insert(pos, old_uv),
                        None => undo_tiles.insert(pos, ui::Rect::NOTHING)
                    }
                }
                else
                {
                    match self.tiles.insert(pos, uv)
                    {
                        Some(old_uv) => undo_tiles.insert(pos, old_uv),
                        None => undo_tiles.insert(pos, ui::Rect::NOTHING)
                    }
                };
            }

            self.undos.add(undo_tiles)
        }

        if app.input.get_key_down(input::KeyCode::KeyS)
            && app.input.get_key_holding(input::KeyCode::ControlLeft)
        {
            let _ = self.save_workspace();
        }
    }

    fn save_workspace(&mut self) -> bincode::Result<()>
    {
        let sprite_sheet = match self.sprite_sheet
        {
            Some(ref sprite_sheet) => sprite_sheet.clone(),
            None =>
            {
                return bincode::Result::Err
                (
                    Box::new(bincode::ErrorKind::Custom("no spritesheet chosen yre".to_owned()))
                )
            }
        };

        // create the savedata struct
        let mut data = SavedData { sprite_sheet, tiles: Vec::new() };

        // here we pass the tiles we have drawn to the vec
        for tile in &self.tiles
        {
            data.tiles.push((*tile.0, *tile.1))
        }

        if self.workspace_path.is_none()
        {
            match rfd::FileDialog::new()
                    .add_filter("", &["bag"])
                    .set_file_name("new tilemap.bag")
                    .save_file()
            {
                Some(path) =>
                {
                    self.workspace_path = Some(path)
                },
                None => return bincode::Result::Err
                (
                    Box::new(bincode::ErrorKind::Custom("failed to crate save file".to_owned()))
                )
            }
        }
        
        let path = self.workspace_path.as_ref().expect
        (
            "this call should have been unreachable as none"
        );

        let mut file = std::fs::File::create(path)?;
        let data = bincode::serialize(&data)?;
        file.write_all(&data)?;
    
        Ok(())
    }
    
    fn load_workspace(&mut self) -> bincode::Result<()>
    {
        let Some(worskspace_path) = rfd::FileDialog::new()
            .add_filter("", &["bag"])
            .set_file_name("load spritesheet data")
            .pick_file()
        else
        {
            return bincode::Result::Err
            (
                Box::new(bincode::ErrorKind::Custom("invalid path".to_owned()))
            )
        };

        let mut file = std::fs::File::open(&worskspace_path)?;

        let mut buf = Vec::new();

        file.read_to_end(&mut buf)?;
    
        match bincode::deserialize::<SavedData>(&buf)
        {
            Ok(SavedData { sprite_sheet, tiles }) =>
            {
                self.tiles.clear();
                self.undos.clear();
                self.redos.clear();

                self.sprite_sheet = Some(sprite_sheet);
                self.workspace_path = Some(worskspace_path);

                for (pos,uv) in tiles
                {
                    self.tiles.insert(pos, uv);
                }
                
                Ok(())
            }
            Err(err) => Err(err)
        }
    }
}

fn load_images<'a>
(
    uri: impl Into<std::borrow::Cow<'a, str>>,
    rows: usize,
    columns: usize
) -> impl Iterator<Item = ui::Image<'a>>
{
    let mut items = Vec::with_capacity(rows * columns);
    
    let image = ui::Image::from_uri(uri);

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

type Tiles = IndexMap<TilePos,ui::Rect>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(serde::Serialize, serde::Deserialize)]
struct TilePos
{
    x: i32, y: i32
}

struct TilesHistory(std::collections::VecDeque<Tiles>, u16);

impl TilesHistory
{
    fn new() -> Self
    {
        Self(Default::default(), 5)
    }

    /// add an undo operation
    fn add(&mut self, tiles: IndexMap<TilePos, ui::Rect>)
    {
        if self.0.len() >= self.1 as usize
        {
            self.0.pop_front();
        }

        self.0.push_back(tiles)
    }

    /// returns the last values added or `None` if the queue has been emptied
    fn pop(&mut self) -> Option<IndexMap<TilePos, ui::Rect>>
    {
        self.0.pop_back()
    }

    fn clear(&mut self)
    {
        self.0.clear()
    }
}

/// contains a path to the spritesheet image, 
/// and the tile data 
#[derive(Serialize,Deserialize)]
struct SavedData
{
    /// the path to the spritesheet used
    sprite_sheet: SpriteSheet,
    tiles: Vec<(TilePos,ui::Rect)>
}