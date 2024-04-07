use std::marker::PhantomData;

use crate::{
    theme::{dynamic_style::DynamicStyle, pseudo_state::PseudoStates, Theme},
    FluxInteraction, TrackedInteraction,
};
use bevy::{
    core::Name,
    ecs::{
        component::{Component, ComponentInfo},
        entity::Entity,
        query::With,
        system::{Command, Commands, EntityCommand, EntityCommands},
        world::{Mut, World},
    },
    hierarchy::{Children, Parent},
    log::{info, warn},
    text::{Text, TextSection, TextStyle},
    ui::{Interaction, UiSurface},
    window::{CursorIcon, PrimaryWindow, Window},
};

struct SetTextSections {
    sections: Vec<TextSection>,
}

impl EntityCommand for SetTextSections {
    fn apply(self, entity: Entity, world: &mut World) {
        let Some(mut text) = world.get_mut::<Text>(entity) else {
            warn!(
                "Failed to set text sections on entity {:?}: No Text component found!",
                entity
            );
            return;
        };

        text.sections = self.sections;
    }
}

pub trait SetTextSectionsExt<'a> {
    fn set_text_sections(&'a mut self, sections: Vec<TextSection>) -> &mut EntityCommands<'a>;
}

impl<'a> SetTextSectionsExt<'a> for EntityCommands<'a> {
    fn set_text_sections(&'a mut self, sections: Vec<TextSection>) -> &mut EntityCommands<'a> {
        self.add(SetTextSections { sections });
        self
    }
}

struct SetText {
    text: String,
    style: TextStyle,
}

impl EntityCommand for SetText {
    fn apply(self, entity: Entity, world: &mut World) {
        let Some(mut text) = world.get_mut::<Text>(entity) else {
            warn!(
                "Failed to set text on entity {:?}: No Text component found!",
                entity
            );
            return;
        };

        text.sections = vec![TextSection::new(self.text, self.style)];
    }
}

pub trait SetTextExt<'a> {
    fn set_text(
        &'a mut self,
        text: impl Into<String>,
        style: Option<TextStyle>,
    ) -> &mut EntityCommands<'a>;
}

impl<'a> SetTextExt<'a> for EntityCommands<'a> {
    fn set_text(
        &'a mut self,
        text: impl Into<String>,
        style: Option<TextStyle>,
    ) -> &mut EntityCommands<'a> {
        self.add(SetText {
            text: text.into(),
            style: style.unwrap_or_default(),
        });

        self
    }
}

// TODO: Move to style and apply to Node's window
struct SetCursor {
    cursor: CursorIcon,
}

impl Command for SetCursor {
    fn apply(self, world: &mut World) {
        let mut q_window = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
        let Ok(mut window) = q_window.get_single_mut(world) else {
            return;
        };

        if window.cursor.icon != self.cursor {
            window.cursor.icon = self.cursor;
        }
    }
}

pub trait SetCursorExt<'w, 's, 'a> {
    fn set_cursor(&'a mut self, cursor: CursorIcon);
}

impl<'w, 's, 'a> SetCursorExt<'w, 's, 'a> for Commands<'w, 's> {
    fn set_cursor(&'a mut self, cursor: CursorIcon) {
        self.add(SetCursor { cursor });
    }
}

struct LogHierarchy {
    level: usize,
    is_last: bool,
    trace_levels: Vec<usize>,
    component_filter: Option<fn(ComponentInfo) -> bool>,
}

impl EntityCommand for LogHierarchy {
    fn apply<'a>(self, id: Entity, world: &mut World) {
        let mut children_ids: Vec<Entity> = Vec::new();
        if let Some(children) = world.get::<Children>(id) {
            children_ids = children.iter().map(|child| *child).collect();
        }

        let filter = self.component_filter;
        let debug_infos: Vec<_> = world
            .inspect_entity(id)
            .into_iter()
            .filter(|component_info| {
                if let Some(filter) = filter {
                    filter((*component_info).clone())
                } else {
                    true
                }
            })
            .map(|component_info| {
                let name = component_info.name();
                let mut simple_name = String::from(name.split("::").last().unwrap());

                if name.split("<").count() > 1 {
                    let left = name.split("<").next().unwrap().split("::").last().unwrap();
                    let generic = name
                        .split("<")
                        .skip(1)
                        .next()
                        .unwrap()
                        .split("::")
                        .last()
                        .unwrap();
                    simple_name = String::new() + left + "<" + generic;
                }

                simple_name
            })
            .collect();

        let prefix = if self.is_last { "╚" } else { "╠" };
        let mut padding_parts: Vec<&str> = Vec::with_capacity(self.level);
        for i in 0..self.level {
            let should_trace = i > 0 && self.trace_levels.contains(&(i - 1));

            padding_parts.push(match should_trace {
                true => "  ║ ",
                false => "    ",
            });
        }

        let padding = padding_parts.join("");
        let name = match world.get::<Name>(id) {
            Some(name) => format!("[{:?}] {}", id, name),
            None => format!("Entity {:?}", id),
        };
        let entity_text = format!("{}  {}══ {} ", padding, prefix, name);
        let has_children = children_ids.len() > 0;

        info!("{}", entity_text);
        for i in 0..debug_infos.len() {
            let is_last = i == (debug_infos.len() - 1);
            let component_pipe = if is_last { "└" } else { "├" };
            let child_pipe = if self.is_last {
                if has_children {
                    "      ║      "
                } else {
                    "             "
                }
            } else {
                if has_children {
                    "  ║   ║      "
                } else {
                    "  ║          "
                }
            };
            info!(
                "{}{}{}── {}",
                padding, child_pipe, component_pipe, debug_infos[i]
            );
        }

        if children_ids.len() > 0 {
            let next_level = self.level + 1;

            for i in 0..children_ids.len() {
                let child = children_ids[i];
                let is_last = i == (children_ids.len() - 1);
                let mut trace_levels = self.trace_levels.clone();
                if !is_last {
                    trace_levels.push(self.level);
                }

                LogHierarchy {
                    level: next_level,
                    is_last,
                    trace_levels,
                    component_filter: self.component_filter,
                }
                .apply(child, world);
            }
        }
    }
}

pub trait LogHierarchyExt<'a> {
    fn log_hierarchy(
        &'a mut self,
        component_filter: Option<fn(ComponentInfo) -> bool>,
    ) -> &mut EntityCommands<'a>;
}

impl<'a> LogHierarchyExt<'a> for EntityCommands<'a> {
    /// Logs the hierarchy of the entity along with the component of each entity in the tree.
    /// Components listed can be optionally filtered by supplying a `component_filter`
    ///
    /// ## Example
    /// ``` rust
    /// commands.entity(parent_id).log_hierarchy(Some(|info| {
    ///     info.name().contains("Node")
    /// }));
    /// ```
    /// ## Output Example
    /// ```
    /// ╚══ Entity 254v2:
    ///     ║      └── Node
    ///     ╠══ Entity 252v2:
    ///     ║   ║      └── Node
    ///     ║   ╚══ Entity 158v2:
    ///     ║       ║      └── Node
    ///     ║       ╠══ Entity 159v2:
    ///     ║       ║   ║      └── Node
    ///     ║       ║   ╚══ Entity 286v1:
    ///     ║       ║              └── Node
    ///     ║       ╚══ Entity 287v1:
    ///     ║                  └── Node
    ///     ╚══ Entity 292v1:
    ///                └── Node
    /// ```
    fn log_hierarchy(
        &'a mut self,
        component_filter: Option<fn(ComponentInfo) -> bool>,
    ) -> &mut EntityCommands<'a> {
        self.add(LogHierarchy {
            level: 0,
            is_last: true,
            trace_levels: vec![],
            component_filter,
        });
        self
    }
}

pub struct ResetChildrenInUiSurface;
impl EntityCommand for ResetChildrenInUiSurface {
    fn apply(self, id: Entity, world: &mut World) {
        world.resource_scope(|world, mut ui_surface: Mut<UiSurface>| {
            let Ok(children) = world.query::<&Children>().get(world, id) else {
                return;
            };
            ui_surface.update_children(id, children);
        });
    }
}

// Adopted from @brandonreinhart
pub trait EntityCommandsNamedExt {
    fn named(&mut self, name: impl Into<String>) -> &mut Self;
}

impl EntityCommandsNamedExt for EntityCommands<'_> {
    fn named(&mut self, name: impl Into<String>) -> &mut Self {
        self.insert(Name::new(name.into()))
    }
}

pub trait RefreshThemeExt<'a> {
    fn refresh_theme<C>(&'a mut self) -> &mut EntityCommands<'a>
    where
        C: Component,
        Theme<C>: Default;
}

impl<'a> RefreshThemeExt<'a> for EntityCommands<'a> {
    fn refresh_theme<C>(&'a mut self) -> &mut EntityCommands<'a>
    where
        C: Component,
        Theme<C>: Default,
    {
        self.add(RefreshEntityTheme::<C> {
            context: PhantomData,
        });
        self
    }
}

struct RefreshEntityTheme<C> {
    context: PhantomData<C>,
}

impl<C> EntityCommand for RefreshEntityTheme<C>
where
    C: Component,
    Theme<C>: Default,
{
    fn apply(self, entity: Entity, world: &mut World) {
        let pseudo_states = world.get::<PseudoStates>(entity);
        let mut style = None;

        // TODO: Move styles from data->builder functions.
        // TODO: Introduce static theme data: Resource<T> (color palette, config values), pass it to builders
        // TODO: Merge generated style with those higher up in the hierarchy to always apply previous themes
        // TODO: Rebuild themes when static data changes
        // TODO: Update flux interaction stopwatch timeout resource
        if let Some(own_theme) = world.get::<Theme<C>>(entity) {
            match pseudo_states {
                Some(pseudo_states) => {
                    style = own_theme.style(pseudo_states);
                }
                None => {
                    style = own_theme.base_style();
                }
            }
        } else {
            let mut found_theme = false;
            let mut current_ancestor = entity;
            while let Some(parent) = world.get::<Parent>(current_ancestor) {
                current_ancestor = parent.get();
                if let Some(ancestor_theme) = world.get::<Theme<C>>(current_ancestor) {
                    match pseudo_states {
                        Some(pseudo_states) => {
                            style = ancestor_theme.style(pseudo_states);
                        }
                        None => {
                            style = ancestor_theme.base_style();
                        }
                    }
                    found_theme = true;
                    break;
                }
            }

            if !found_theme {
                let theme = Theme::<C>::default();
                match pseudo_states {
                    Some(pseudo_states) => {
                        style = theme.style(pseudo_states);
                    }
                    None => {
                        style = theme.base_style();
                    }
                }
            }
        }

        if let Some(style) = style {
            if style.is_interactive() || style.is_animated() {
                world.entity_mut(entity).insert(style);
                if world.get::<Interaction>(entity).is_none() {
                    world.entity_mut(entity).insert(Interaction::default());
                }
                if world.get::<FluxInteraction>(entity).is_none() {
                    world
                        .entity_mut(entity)
                        .insert(TrackedInteraction::default());
                }
            } else {
                world.entity_mut(entity).insert(style);
            }
        } else {
            world.entity_mut(entity).remove::<DynamicStyle>();
        }
    }
}
