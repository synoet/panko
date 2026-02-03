//! Context-based keymap system inspired by Zed editor.
//!
//! Bindings are matched against a context stack, with more specific contexts winning.
//! Example: CommentFocused > DiffView > Global

use crate::ports::{KeyCode, KeyModifiers};

/// Contexts that can be active. Forms a specificity hierarchy.
/// More specific contexts (higher discriminant) take precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Context {
    /// Always active - lowest precedence
    Global = 0,
    /// File tree has focus
    FileTree = 1,
    /// Diff view has focus
    DiffView = 2,
    /// Filter input has focus
    FilterInput = 3,
    /// Visual selection mode
    Visual = 4,
    /// Comment input mode (new comment or reply)
    CommentInput = 5,
    /// A comment is focused (can reply, resolve, delete)
    CommentFocused = 6,
    /// Help overlay is shown
    Help = 7,
    /// Theme picker overlay is shown
    ThemePicker = 8,
    /// Fuzzy search overlay is shown
    FuzzySearch = 9,
}

/// Categories for grouping keybindings in help display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HelpCategory {
    Navigation = 0,
    Actions = 1,
    Comments = 2,
    General = 3,
}

impl HelpCategory {
    pub fn display_name(self) -> &'static str {
        match self {
            HelpCategory::Navigation => "Navigation",
            HelpCategory::Actions => "Actions",
            HelpCategory::Comments => "Comments",
            HelpCategory::General => "General",
        }
    }
}

impl Context {
    /// Specificity for precedence ordering. Higher = more specific.
    pub fn specificity(self) -> u8 {
        self as u8
    }
}

/// Actions that can be triggered by key bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Navigation
    MoveDown,
    MoveUp,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    GotoTop,
    GotoBottom,
    NextFile,
    PrevFile,

    // Focus
    SwitchPane,
    FocusFileTree,
    FocusDiffView,
    FocusFilter,

    // View toggles
    ToggleSplitView,
    ToggleSidebar,
    ToggleComments,
    CycleDiffSource,

    // File actions
    ToggleCollapse,
    ToggleViewed,
    SelectFile,

    // Comment actions
    EnterVisualMode,
    ExitVisualMode,
    StartComment,
    SubmitInput,
    CancelInput,
    ReplyToComment,
    ToggleResolved,
    DeleteComment,

    // Filter input
    FilterBackspace,

    // General
    Refresh,
    ShowHelp,
    DismissHelp,
    ToggleThemePicker,
    Quit,

    // Input mode
    InputBackspace,

    // Theme picker
    ApplyTheme,
    CloseThemePicker,

    // Fuzzy search
    OpenFuzzySearch,
    CloseFuzzySearch,
    FuzzySearchSelect,
    FuzzySearchBackspace,
}

/// A single key binding with optional context requirement.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
    /// If Some, binding only active in this context. None = Global.
    pub context: Option<Context>,
    pub action: Action,
    /// Description for help display. If None, binding is hidden from help.
    pub help_text: Option<&'static str>,
    /// Category for grouping in help display.
    pub category: Option<HelpCategory>,
}

impl KeyBinding {
    pub fn new(key: KeyCode, action: Action) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::default(),
            context: None,
            action,
            help_text: None,
            category: None,
        }
    }

    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }


    pub fn in_context(mut self, ctx: Context) -> Self {
        self.context = Some(ctx);
        self
    }

    /// Add help text and category for display in help menu.
    pub fn help(mut self, category: HelpCategory, text: &'static str) -> Self {
        self.category = Some(category);
        self.help_text = Some(text);
        self
    }
}

/// The keymap holds all bindings and dispatches key events.
pub struct Keymap {
    bindings: Vec<KeyBinding>,
}

impl Keymap {
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    /// Add a binding. Later bindings take precedence at same specificity.
    pub fn bind(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Look up the action for a key event given active contexts.
    /// Returns the action from the most specific matching context.
    pub fn lookup(
        &self,
        key: KeyCode,
        modifiers: KeyModifiers,
        active_contexts: &[Context],
    ) -> Option<Action> {
        let mut best_match: Option<(u8, Action)> = None;

        // Iterate in reverse so later bindings win at same specificity
        for binding in self.bindings.iter().rev() {
            // Check key match
            if !self.key_matches(binding, key, &modifiers) {
                continue;
            }

            // Check context match and get specificity
            let specificity = match binding.context {
                None => {
                    // Global binding - always matches with specificity 0
                    0
                }
                Some(ctx) => {
                    // Context-specific binding - must be in active contexts
                    if active_contexts.contains(&ctx) {
                        ctx.specificity()
                    } else {
                        continue; // Context not active, skip
                    }
                }
            };

            // Update best match if this is more specific
            match best_match {
                None => best_match = Some((specificity, binding.action)),
                Some((best_spec, _)) if specificity > best_spec => {
                    best_match = Some((specificity, binding.action));
                }
                _ => {} // Keep existing better match
            }
        }

        best_match.map(|(_, action)| action)
    }

    fn key_matches(&self, binding: &KeyBinding, key: KeyCode, modifiers: &KeyModifiers) -> bool {
        // For char keys, we need special handling
        match (&binding.key, &key) {
            (KeyCode::Char(a), KeyCode::Char(b)) => {
                // Case-sensitive match for letters
                a == b && binding.modifiers.ctrl == modifiers.ctrl
            }
            _ => binding.key == key && binding.modifiers.ctrl == modifiers.ctrl,
        }
    }

    /// Generate help entries grouped by category.
    pub fn help_entries(&self) -> Vec<(HelpCategory, Vec<HelpEntry>)> {
        use std::collections::BTreeMap;

        let mut by_category: BTreeMap<HelpCategory, Vec<HelpEntry>> = BTreeMap::new();
        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

        for binding in &self.bindings {
            if let (Some(category), Some(text)) = (binding.category, binding.help_text) {
                let key_display = format_key_display(&binding.key, &binding.modifiers);

                // Deduplicate by key display string (first one wins)
                if seen_keys.contains(&key_display) {
                    continue;
                }
                seen_keys.insert(key_display.clone());

                by_category.entry(category).or_default().push(HelpEntry {
                    key_display,
                    description: text,
                    context_hint: binding.context.map(format_context_hint),
                });
            }
        }

        by_category.into_iter().collect()
    }
}

/// Entry for help display.
#[derive(Debug, Clone)]
pub struct HelpEntry {
    pub key_display: String,
    pub description: &'static str,
    pub context_hint: Option<&'static str>,
}

fn format_key_display(key: &KeyCode, modifiers: &KeyModifiers) -> String {
    let mut parts = Vec::new();

    if modifiers.ctrl {
        parts.push("Ctrl+".to_string());
    }

    let key_str = match key {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        _ => "?".to_string(),
    };

    parts.push(key_str);
    parts.concat()
}

fn format_context_hint(ctx: Context) -> &'static str {
    match ctx {
        Context::Visual => "visual",
        Context::CommentFocused => "comment",
        Context::FilterInput => "filter",
        Context::CommentInput => "input",
        _ => "",
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default keymap with all bindings.
pub fn build_default_keymap() -> Keymap {
    use HelpCategory::*;

    let mut km = Keymap::new();

    // Helper to reduce verbosity
    let key = |k: KeyCode, a: Action| KeyBinding::new(k, a);
    let ch = |c: char, a: Action| KeyBinding::new(KeyCode::Char(c), a);

    // === Navigation (shown in help) ===
    km.bind(ch('j', Action::MoveDown).help(Navigation, "Move down"));
    km.bind(ch('k', Action::MoveUp).help(Navigation, "Move up"));
    km.bind(ch('d', Action::HalfPageDown).with_ctrl().help(Navigation, "Half page down"));
    km.bind(ch('u', Action::HalfPageUp).with_ctrl().help(Navigation, "Half page up"));
    km.bind(ch('g', Action::GotoTop).help(Navigation, "Go to top"));
    km.bind(ch('G', Action::GotoBottom).help(Navigation, "Go to bottom"));
    km.bind(ch('n', Action::NextFile).help(Navigation, "Next file"));
    km.bind(ch('p', Action::PrevFile).help(Navigation, "Previous file"));

    // === Actions (shown in help) ===
    km.bind(key(KeyCode::Enter, Action::SelectFile).help(Actions, "Select file / toggle collapse"));
    km.bind(key(KeyCode::Tab, Action::SwitchPane).help(Actions, "Switch pane focus"));
    km.bind(ch('1', Action::FocusFileTree).help(Actions, "Focus file tree"));
    km.bind(ch('2', Action::FocusDiffView).help(Actions, "Focus diff view"));
    km.bind(ch('/', Action::FocusFilter).help(Actions, "Focus filter input"));
    km.bind(ch('x', Action::ToggleViewed).help(Actions, "Mark file as viewed"));
    km.bind(ch('c', Action::ToggleCollapse).help(Actions, "Collapse/expand file"));
    km.bind(ch('s', Action::ToggleSplitView).help(Actions, "Toggle split/unified view"));
    km.bind(ch('!', Action::ToggleSidebar).help(Actions, "Toggle sidebar"));
    km.bind(ch('b', Action::ToggleSidebar)); // Keep as secondary binding (hidden from help)
    km.bind(ch('r', Action::Refresh).help(Actions, "Refresh diff"));
    km.bind(ch('u', Action::CycleDiffSource).help(Actions, "Cycle diff source"));

    // === Comments (shown in help) ===
    km.bind(ch('v', Action::EnterVisualMode).help(Comments, "Enter visual mode"));
    km.bind(ch('c', Action::StartComment).in_context(Context::Visual).help(Comments, "Add comment on selection"));
    km.bind(ch('r', Action::ReplyToComment).in_context(Context::CommentFocused).help(Comments, "Reply to comment"));
    km.bind(ch('R', Action::ToggleResolved).help(Comments, "Toggle comment resolved"));
    km.bind(ch('D', Action::DeleteComment).in_context(Context::CommentFocused).help(Comments, "Delete comment"));
    km.bind(ch('C', Action::ToggleComments).help(Comments, "Show/hide comments"));

    // === General (shown in help) ===
    km.bind(ch('?', Action::ShowHelp).help(General, "Toggle help"));
    km.bind(ch('t', Action::ToggleThemePicker).help(General, "Theme picker"));
    km.bind(ch('q', Action::Quit).help(General, "Quit"));

    // === Additional bindings (not shown in help - duplicates or internal) ===
    // Arrow key alternatives
    km.bind(key(KeyCode::Down, Action::MoveDown));
    km.bind(key(KeyCode::Up, Action::MoveUp));
    km.bind(key(KeyCode::PageDown, Action::PageDown));
    km.bind(key(KeyCode::PageUp, Action::PageUp));

    // Ctrl+c to quit
    km.bind(ch('c', Action::Quit).with_ctrl());

    // Context-specific navigation (same keys, different contexts)
    km.bind(key(KeyCode::Down, Action::MoveDown).in_context(Context::FileTree));
    km.bind(ch('j', Action::MoveDown).in_context(Context::FileTree));
    km.bind(key(KeyCode::Up, Action::MoveUp).in_context(Context::FileTree));
    km.bind(ch('k', Action::MoveUp).in_context(Context::FileTree));
    km.bind(ch('g', Action::GotoTop).in_context(Context::FileTree));
    km.bind(ch('G', Action::GotoBottom).in_context(Context::FileTree));
    km.bind(key(KeyCode::Enter, Action::SelectFile).in_context(Context::FileTree));
    km.bind(ch('c', Action::ToggleCollapse).in_context(Context::FileTree));
    km.bind(ch('x', Action::ToggleViewed).in_context(Context::FileTree));

    km.bind(key(KeyCode::Down, Action::MoveDown).in_context(Context::DiffView));
    km.bind(ch('j', Action::MoveDown).in_context(Context::DiffView));
    km.bind(key(KeyCode::Up, Action::MoveUp).in_context(Context::DiffView));
    km.bind(ch('k', Action::MoveUp).in_context(Context::DiffView));
    km.bind(key(KeyCode::PageDown, Action::PageDown).in_context(Context::DiffView));
    km.bind(ch('d', Action::PageDown).in_context(Context::DiffView));
    km.bind(key(KeyCode::PageUp, Action::PageUp).in_context(Context::DiffView));
    km.bind(ch('g', Action::GotoTop).in_context(Context::DiffView));
    km.bind(ch('G', Action::GotoBottom).in_context(Context::DiffView));
    km.bind(ch('n', Action::NextFile).in_context(Context::DiffView));
    km.bind(ch('p', Action::PrevFile).in_context(Context::DiffView));
    km.bind(ch('c', Action::ToggleCollapse).in_context(Context::DiffView));
    km.bind(ch('x', Action::ToggleViewed).in_context(Context::DiffView));
    km.bind(ch('v', Action::EnterVisualMode).in_context(Context::DiffView));
    km.bind(key(KeyCode::Enter, Action::ToggleCollapse).in_context(Context::DiffView));
    km.bind(ch('R', Action::ToggleResolved).in_context(Context::DiffView));

    // Comment focused context
    km.bind(ch('R', Action::ToggleResolved).in_context(Context::CommentFocused));

    // === Help mode - highest precedence for dismissal ===
    km.bind(key(KeyCode::Esc, Action::DismissHelp).in_context(Context::Help));
    km.bind(ch('?', Action::DismissHelp).in_context(Context::Help));
    km.bind(ch('q', Action::DismissHelp).in_context(Context::Help));
    km.bind(key(KeyCode::Enter, Action::DismissHelp).in_context(Context::Help));

    // === Theme picker mode ===
    km.bind(key(KeyCode::Esc, Action::CloseThemePicker).in_context(Context::ThemePicker));
    km.bind(ch('q', Action::CloseThemePicker).in_context(Context::ThemePicker));
    km.bind(key(KeyCode::Enter, Action::ApplyTheme).in_context(Context::ThemePicker));

    // === Filter input mode ===
    km.bind(key(KeyCode::Esc, Action::CancelInput).in_context(Context::FilterInput));
    km.bind(key(KeyCode::Enter, Action::SubmitInput).in_context(Context::FilterInput));
    km.bind(key(KeyCode::Backspace, Action::FilterBackspace).in_context(Context::FilterInput));

    // === Visual mode ===
    km.bind(key(KeyCode::Esc, Action::ExitVisualMode).in_context(Context::Visual));
    km.bind(key(KeyCode::Down, Action::MoveDown).in_context(Context::Visual));
    km.bind(ch('j', Action::MoveDown).in_context(Context::Visual));
    km.bind(key(KeyCode::Up, Action::MoveUp).in_context(Context::Visual));
    km.bind(ch('k', Action::MoveUp).in_context(Context::Visual));
    km.bind(key(KeyCode::Enter, Action::StartComment).in_context(Context::Visual));

    // === Comment input mode ===
    km.bind(key(KeyCode::Esc, Action::CancelInput).in_context(Context::CommentInput));
    km.bind(key(KeyCode::Enter, Action::SubmitInput).in_context(Context::CommentInput));
    km.bind(key(KeyCode::Backspace, Action::InputBackspace).in_context(Context::CommentInput));

    // === Fuzzy search mode ===
    // '/' in DiffView opens fuzzy search (higher specificity than Global FocusFilter)
    km.bind(ch('/', Action::OpenFuzzySearch).in_context(Context::DiffView));
    km.bind(key(KeyCode::Esc, Action::CloseFuzzySearch).in_context(Context::FuzzySearch));
    km.bind(key(KeyCode::Enter, Action::FuzzySearchSelect).in_context(Context::FuzzySearch));
    km.bind(key(KeyCode::Backspace, Action::FuzzySearchBackspace).in_context(Context::FuzzySearch));
    km.bind(ch('j', Action::MoveDown).in_context(Context::FuzzySearch));
    km.bind(ch('k', Action::MoveUp).in_context(Context::FuzzySearch));
    km.bind(key(KeyCode::Down, Action::MoveDown).in_context(Context::FuzzySearch));
    km.bind(key(KeyCode::Up, Action::MoveUp).in_context(Context::FuzzySearch));
    km.bind(ch('n', Action::MoveDown).with_ctrl().in_context(Context::FuzzySearch));
    km.bind(ch('p', Action::MoveUp).with_ctrl().in_context(Context::FuzzySearch));

    km
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_binding() {
        let km = build_default_keymap();
        let contexts = vec![Context::Global, Context::DiffView];

        let action = km.lookup(KeyCode::Char('q'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn test_context_specific_binding() {
        let km = build_default_keymap();
        let contexts = vec![Context::Global, Context::DiffView];

        let action = km.lookup(KeyCode::Char('j'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::MoveDown));
    }

    #[test]
    fn test_more_specific_context_wins() {
        let km = build_default_keymap();

        // Without CommentFocused, 'r' triggers Refresh
        let contexts_no_focus = vec![Context::Global, Context::DiffView];
        let action = km.lookup(KeyCode::Char('r'), KeyModifiers::default(), &contexts_no_focus);
        assert_eq!(action, Some(Action::Refresh));

        // With CommentFocused, 'r' triggers ReplyToComment
        let contexts_focused = vec![Context::Global, Context::DiffView, Context::CommentFocused];
        let action = km.lookup(KeyCode::Char('r'), KeyModifiers::default(), &contexts_focused);
        assert_eq!(action, Some(Action::ReplyToComment));
    }

    #[test]
    fn test_ctrl_modifier() {
        let km = build_default_keymap();
        let contexts = vec![Context::Global, Context::DiffView];

        // 'u' without ctrl = CycleDiffSource
        let action = km.lookup(KeyCode::Char('u'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::CycleDiffSource));

        // Ctrl+u = HalfPageUp
        let mut mods = KeyModifiers::default();
        mods.ctrl = true;
        let action = km.lookup(KeyCode::Char('u'), mods, &contexts);
        assert_eq!(action, Some(Action::HalfPageUp));
    }

    #[test]
    fn test_help_mode_captures_quit() {
        let km = build_default_keymap();

        // In Help context, 'q' dismisses help instead of quitting
        let contexts = vec![Context::Global, Context::Help];
        let action = km.lookup(KeyCode::Char('q'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::DismissHelp));
    }

    #[test]
    fn test_help_entries_generated() {
        let km = build_default_keymap();
        let entries = km.help_entries();

        // Should have 4 categories
        assert_eq!(entries.len(), 4);

        // Categories should be in order
        assert_eq!(entries[0].0, HelpCategory::Navigation);
        assert_eq!(entries[1].0, HelpCategory::Actions);
        assert_eq!(entries[2].0, HelpCategory::Comments);
        assert_eq!(entries[3].0, HelpCategory::General);

        // Navigation should have entries
        assert!(!entries[0].1.is_empty());

        // Check a specific entry
        let nav_entries = &entries[0].1;
        let move_down = nav_entries.iter().find(|e| e.description == "Move down");
        assert!(move_down.is_some());
        assert_eq!(move_down.unwrap().key_display, "j");
    }

    #[test]
    fn test_fuzzy_search_in_diff_view() {
        let km = build_default_keymap();

        // '/' in DiffView context should open fuzzy search
        let contexts = vec![Context::Global, Context::DiffView];
        let action = km.lookup(KeyCode::Char('/'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::OpenFuzzySearch));

        // '/' in FileTree context should focus filter (falls through to Global)
        let contexts = vec![Context::Global, Context::FileTree];
        let action = km.lookup(KeyCode::Char('/'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::FocusFilter));
    }

    #[test]
    fn test_fuzzy_search_mode_captures_keys() {
        let km = build_default_keymap();
        let contexts = vec![Context::Global, Context::DiffView, Context::FuzzySearch];

        // Esc closes fuzzy search
        let action = km.lookup(KeyCode::Esc, KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::CloseFuzzySearch));

        // Enter selects result
        let action = km.lookup(KeyCode::Enter, KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::FuzzySearchSelect));

        // j/k navigate
        let action = km.lookup(KeyCode::Char('j'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::MoveDown));

        let action = km.lookup(KeyCode::Char('k'), KeyModifiers::default(), &contexts);
        assert_eq!(action, Some(Action::MoveUp));
    }
}
