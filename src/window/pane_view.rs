//! Window-local presentation only. The shared model and running terminals never change.
use crate::{model::Model, mods::Instance};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) struct PaneView {
    terminal: String,
    pub inst: Instance,
    pub active: bool,
}

#[derive(Default)]
pub(super) struct PaneViews(pub HashMap<String, PaneView>);

impl PaneViews {
    pub fn active(&self, pane: &str) -> Option<&Instance> {
        self.0.get(pane).filter(|v| v.active).map(|v| &v.inst)
    }

    pub fn select(
        &mut self,
        model: &Model,
        pane: &str,
        name: Option<&str>,
        props: &Value,
    ) -> Result<(), String> {
        let terminal = model
            .address_of(pane)
            .ok_or("pane.view needs a terminal pane")?;
        if let Some(name) = name {
            if name.is_empty() {
                return Err("pane.view mod must not be empty".into());
            }
            if !props.is_null() && !props.is_object() {
                return Err("pane.view props must be an object".into());
            }
            let mut props = if props.is_null() {
                json!({})
            } else {
                props.clone()
            };
            // Identity always comes from the model, including machine-qualified addresses.
            props["pane"] = json!(pane);
            props["terminal"] = json!(terminal);
            self.0.insert(
                pane.to_owned(),
                PaneView {
                    terminal,
                    inst: Instance {
                        name: name.to_owned(),
                        props,
                    },
                    active: true,
                },
            );
        } else if let Some(view) = self.0.get_mut(pane) {
            view.active = false; // keep the mod's draft and scroll position
        }
        Ok(())
    }

    pub fn retain(&mut self, model: &Model) {
        self.0
            .retain(|pane, view| model.address_of(pane).as_deref() == Some(&view.terminal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        layout::{Axis, Content, Node},
        model::Tab,
    };

    fn split_model() -> Model {
        let mut m = Model::default();
        m.add_space("space", "", "", Tab::new("tab", "", "left", "t1"));
        m.split("left", Axis::Row, "right", "remote/t2");
        m
    }

    #[test]
    fn independent_views_preserve_model_and_restore_retained_instances() {
        let m = split_model();
        let before = serde_json::to_value(&m.active_tab().unwrap().root).unwrap();
        let focused = m.focused_pane();
        let mut views = PaneViews::default();
        views
            .select(&m, "left", Some("chat"), &json!({"terminal":"wrong"}))
            .unwrap();
        views
            .select(&m, "right", Some("chat"), &Value::Null)
            .unwrap();
        assert_eq!(
            views.active("right").unwrap().props["terminal"],
            "remote/t2"
        );
        assert_eq!(views.active("left").unwrap().props["terminal"], "t1");
        let inst = views.active("left").unwrap().clone();
        views.select(&m, "left", None, &Value::Null).unwrap();
        assert!(views.active("left").is_none());
        assert!(views.active("right").is_some());
        assert!(views.0["left"].inst == inst);
        views
            .select(&m, "left", Some("chat"), &Value::Null)
            .unwrap();
        assert!(views.active("left").unwrap() == &inst);
        assert_eq!(
            serde_json::to_value(&m.active_tab().unwrap().root).unwrap(),
            before
        );
        assert_eq!(m.focused_pane(), focused);
        assert!(m.active_tab().unwrap().zoomed.is_none());
    }

    #[test]
    fn invalid_targets_and_props_do_not_change_view_selection() {
        let mut m = split_model();
        m.split_with(
            "right",
            Axis::Column,
            Node::block(
                "mod",
                Content::Mod {
                    name: "files".into(),
                    props: Value::Null,
                },
            ),
        );
        let mut views = PaneViews::default();
        for pane in ["missing", "mod"] {
            assert!(views.select(&m, pane, Some("chat"), &Value::Null).is_err());
        }
        assert!(views.select(&m, "left", Some("chat"), &json!([])).is_err());
        assert!(views.select(&m, "left", Some(""), &Value::Null).is_err());
        assert!(views.0.is_empty());
    }

    #[test]
    fn closing_or_replacing_a_terminal_discards_its_retained_chat() {
        let mut m = split_model();
        let mut views = PaneViews::default();
        views
            .select(&m, "left", Some("chat"), &Value::Null)
            .unwrap();
        views
            .select(&m, "right", Some("chat"), &Value::Null)
            .unwrap();
        m.remove_pane("left");
        views.retain(&m);
        assert!(!views.0.contains_key("left"));
        assert!(views.active("right").is_some());
        m.remove_pane("right");
        views.retain(&m);
        assert!(views.0.is_empty());
    }
}
