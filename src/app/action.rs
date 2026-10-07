//! Every state change in the app is an `Action`. Key events, commands and
//! (later) remote events all reduce to this enum and go through
//! `App::dispatch`, which keeps the UI layer purely presentational.

use super::misses::MissedRange;
use crate::catalog::CatalogEvent;
use crate::online::RemoteEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Typing screen
    TypeChar(char),
    /// `:pm`, before the first key: fewer (-1) or more (1) missed words.
    PracticeStep(i8),
    Backspace,
    DeleteWord,
    Restart,

    // Command line editing
    OpenCommandLine,
    CloseCommandLine,
    CmdInsert(char),
    CmdBackspace,
    CmdDeleteWord,
    CmdLeft,
    CmdRight,
    CmdHome,
    CmdEnd,
    CmdSelectNext,
    CmdSelectPrev,
    CmdAccept,
    CmdSubmit,

    // Navigation
    /// Leave stats/help and return to the screen underneath.
    Back,
    ShowStats,
    ShowHelp,
    ScrollDown,
    ScrollUp,
    /// History screen: move the selected run.
    History(ListMove),
    /// Open the missed-keys screen, on a range or the last one shown.
    ShowMissed(Option<MissedRange>),
    /// Missed-keys screen: the next (1) or previous (-1) range.
    MissedStep(i8),

    // Landing screen
    /// Finish the intro animation at once.
    SkipIntro,
    /// Leave the landing screen for the typing screen.
    CloseSplash,
    /// Landing menu: move the highlight.
    SplashMove(i8),
    /// Landing menu: run the highlighted row.
    SplashChoose,
    /// A key while the screensaver types: crumble it.
    IdleWake,

    // Results screen
    /// A key while the new-best confetti plays: end it.
    EndCelebration,

    // Housekeeping
    Tick,
    Redraw,
    FontBigger,
    FontSmaller,

    // Slider (bottom-line numeric picker)
    SliderDec,
    SliderInc,
    SliderMin,
    SliderMax,
    SliderConfirm,
    SliderCancel,

    // Profile menu
    ShowProfiles,
    Profile(ProfileAction),

    // Install menu (languages and themes from the catalogue)
    ShowCatalog,
    Catalog(CatalogAction),
    /// Keys on the modules checklist.
    Modules(ModuleAction),
    /// Open the custom page (the install screen's custom tab).
    ShowCustom,
    /// Keys on the custom page.
    Custom(CustomAction),
    /// Keys in the custom set editor.
    CustomEdit(EditAction),
    /// A catalogue download finished.
    CatalogFetched(Box<CatalogEvent>),

    // Online (only when `server` is configured)
    Login,
    CancelLogin,
    Logout,
    ShowLeaderboard,
    /// `:daily` with the configured mode.
    Daily,
    Board(BoardAction),
    /// Keys on a user's profile screen.
    User(UserAction),
    /// Keys on the players screen (search and follow list).
    Players(PlayersAction),
    /// A reply from a background network request (boxed: replies carry
    /// whole leaderboards, and every other action is a few bytes).
    Remote(Box<RemoteEvent>),

    Quit,
    Nop,
}

/// Cursor movement in a plain list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMove {
    By(isize),
    Page(isize),
    Home,
    End,
}

/// Keys on the leaderboard screen and its graph view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardAction {
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    /// Focus the next board (side by side) or show it (tabs).
    SwitchBoard,
    /// Show the next period's boards (daily, all time, …).
    NextPeriod,
    NextMode,
    PrevMode,
    NextLanguage,
    PrevDay,
    NextDay,
    /// Open the selected run's graph.
    Open,
    /// Back from the graph to the screen that opened it, same row selected.
    CloseGraph,
    /// Open the profile of the selected row's user.
    Profile,
    /// Everyone ↔ only you and the players you follow.
    ToggleFollowing,
}

/// Keys on a user's profile screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    Up,
    Down,
    Top,
    Bottom,
    /// Open the selected recent run's graph.
    Open,
    /// Follow or unfollow this player.
    Follow,
    /// Back to where the profile was opened from.
    Close,
}

/// Keys on the players screen (`:search`, `:follow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayersAction {
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    /// Open the selected player's profile.
    Open,
    /// Follow or unfollow the selected player.
    Follow,
    /// Search ↔ following.
    SwitchTab,
    /// Start typing into the search.
    Edit,
    /// Stop typing; letters are keys again.
    StopEditing,
    SearchChar(char),
    SearchBackspace,
    SearchDeleteWord,
    /// Back to where the screen was opened from.
    Close,
}

/// Keys on the custom page (`Screen::Custom`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomAction {
    Up,
    Down,
    Top,
    Bottom,
    /// Type the selected set (installing it first if it's shared).
    Use,
    New,
    Edit,
    /// Ask to delete the selected set from this machine.
    Remove,
    /// Publish (or update) the selected set of yours.
    Publish,
    /// Ask to take the selected set of yours off the server.
    Unpublish,
    Yes,
    No,
    /// Start typing a search.
    Search,
    StopSearch,
    SearchChar(char),
    SearchBackspace,
    /// On to the install screen's languages tab.
    SwitchTab,
    Close,
}

/// Keys in the custom set editor (`Screen::CustomEdit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    Char(char),
    Backspace,
    DeleteWord,
    /// Take the name, or ask to add the typed words.
    Enter,
    /// Add the words asked about.
    Yes,
    /// Don't add them; keep the typing.
    No,
    /// Typing ↔ the word list.
    ToggleFocus,
    Up,
    Down,
    /// Remove the word under the cursor.
    Remove,
    /// Clear the typing, or leave when it's empty.
    Escape,
}

/// Keys on the modules checklist (`Screen::Modules`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleAction {
    Move(isize),
    Top,
    Bottom,
    Toggle,
    ToggleAll,
    Apply,
    Cancel,
}

/// Keys on the install screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAction {
    Up,
    Down,
    Top,
    Bottom,
    /// Languages ↔ themes.
    SwitchTab,
    /// Make the selected item current, installing it first if needed.
    Use,
    /// Install without switching to it.
    Install,
    /// Ask to remove the selected item.
    Remove,
    ConfirmRemove,
    CancelRemove,
    /// Fetch the catalogue index again.
    Refresh,
    /// Add a character to the search.
    SearchChar(char),
    /// Drop the search's last character.
    SearchBackspace,
    /// Drop the search's last word.
    SearchDeleteWord,
    /// Clear the search, or leave when it's already empty.
    Escape,
}

/// Keys on the profile screen, in either the list or the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    Up,
    Down,
    /// List: switch the selected profile on or off (or create, on the last row).
    Toggle,
    New,
    Edit,
    /// List: ask to delete the selected profile.
    Delete,
    ConfirmDelete,
    CancelDelete,
    /// Editor: check or uncheck the focused setting.
    Check,
    /// Editor: step the focused setting's value back (-1) or forward (1).
    Cycle(i8),
    /// Editor: take the focused setting's live value.
    Capture,
    /// Editor: take every setting's live value.
    CaptureAll,
    Insert(char),
    Backspace,
    Save,
    /// Editor: discard changes and return to the list.
    Cancel,
}
