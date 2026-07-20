use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, PartialEq, Debug)]
pub enum WidgetType {
    Container,
    Button,
    Label,
}

pub struct Widget {
    pub id: i64,
    pub widget_type: WidgetType,
    pub parent_id: Option<i64>,
    pub text: String,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub clicked: bool,
    pub children: Vec<i64>,
}

struct GuiState {
    initialized: bool,
    next_id: i64,
    widgets: HashMap<i64, Widget>,
}

impl GuiState {
    fn new() -> Self {
        Self {
            initialized: false,
            next_id: 1,
            widgets: HashMap::new(),
        }
    }
}

static GUI_STATE: OnceLock<Mutex<GuiState>> = OnceLock::new();

fn get_gui_state() -> &'static Mutex<GuiState> {
    GUI_STATE.get_or_init(|| Mutex::new(GuiState::new()))
}

pub fn init() -> bool {
    if let Ok(mut state) = get_gui_state().lock() {
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn create_container(title: &str, width: i64, height: i64) -> i64 {
    if let Ok(mut state) = get_gui_state().lock() {
        if !state.initialized {
            return -1;
        }
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        state.widgets.insert(id, Widget {
            id,
            widget_type: WidgetType::Container,
            parent_id: None,
            text: title.to_string(),
            x: 0,
            y: 0,
            width,
            height,
            clicked: false,
            children: Vec::new(),
        });
        id
    } else {
        -1
    }
}

pub fn add_button(parent_id: i64, label: &str, x: i64, y: i64, width: i64, height: i64) -> i64 {
    if let Ok(mut state) = get_gui_state().lock() {
        if !state.initialized || !state.widgets.contains_key(&parent_id) {
            return -1;
        }
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        let widget = Widget {
            id,
            widget_type: WidgetType::Button,
            parent_id: Some(parent_id),
            text: label.to_string(),
            x,
            y,
            width,
            height,
            clicked: false,
            children: Vec::new(),
        };
        state.widgets.insert(id, widget);
        if let Some(parent) = state.widgets.get_mut(&parent_id) {
            parent.children.push(id);
        }
        id
    } else {
        -1
    }
}

pub fn add_label(parent_id: i64, text: &str, x: i64, y: i64) -> i64 {
    if let Ok(mut state) = get_gui_state().lock() {
        if !state.initialized || !state.widgets.contains_key(&parent_id) {
            return -1;
        }
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        let widget = Widget {
            id,
            widget_type: WidgetType::Label,
            parent_id: Some(parent_id),
            text: text.to_string(),
            x,
            y,
            width: 100,
            height: 24,
            clicked: false,
            children: Vec::new(),
        };
        state.widgets.insert(id, widget);
        if let Some(parent) = state.widgets.get_mut(&parent_id) {
            parent.children.push(id);
        }
        id
    } else {
        -1
    }
}

pub fn set_text(widget_id: i64, new_text: &str) -> bool {
    if let Ok(mut state) = get_gui_state().lock() {
        if let Some(widget) = state.widgets.get_mut(&widget_id) {
            widget.text = new_text.to_string();
            return true;
        }
    }
    false
}

pub fn get_text(widget_id: i64) -> String {
    if let Ok(state) = get_gui_state().lock() {
        if let Some(widget) = state.widgets.get(&widget_id) {
            return widget.text.clone();
        }
    }
    String::new()
}

pub fn trigger_click(widget_id: i64) -> bool {
    if let Ok(mut state) = get_gui_state().lock() {
        if let Some(widget) = state.widgets.get_mut(&widget_id) {
            widget.clicked = true;
            return true;
        }
    }
    false
}

pub fn is_clicked(widget_id: i64) -> bool {
    if let Ok(mut state) = get_gui_state().lock() {
        if let Some(widget) = state.widgets.get_mut(&widget_id) {
            let was_clicked = widget.clicked;
            widget.clicked = false;
            return was_clicked;
        }
    }
    false
}

pub fn child_count(parent_id: i64) -> usize {
    if let Ok(state) = get_gui_state().lock() {
        if let Some(widget) = state.widgets.get(&parent_id) {
            return widget.children.len();
        }
    }
    0
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_gui_state().lock() {
        state.widgets.clear();
        state.initialized = false;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_widget_tree_and_events() {
        assert!(init());
        let root = create_container("Main Window", 800, 600);
        assert!(root > 0);
        
        let btn = add_button(root, "Click Me", 10, 10, 120, 40);
        assert!(btn > 0);
        let lbl = add_label(root, "Status: Ready", 10, 60);
        assert!(lbl > 0);
        
        assert_eq!(child_count(root), 2);
        assert!(set_text(lbl, "Status: Active"));
        assert_eq!(get_text(lbl), "Status: Active");
        
        assert!(!is_clicked(btn));
        assert!(trigger_click(btn));
        assert!(is_clicked(btn));
        assert!(!is_clicked(btn));
        
        assert!(shutdown());
    }
}
