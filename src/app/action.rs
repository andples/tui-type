//! Every state change in the app is an `Action`. Key events, commands and
//! (later) remote events all reduce to this enum and go through
//! `App::dispatch`, which keeps the UI layer purely presentational.

use crate::catalog::CatalogEvent;
use crate::online::RemoteEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Typing screen
    TypeChar(char),
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
    /// Focus the other board (side by side) or show it (tabs).
    SwitchBoard,
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
    /// Back to where the profile was opened from.
    Close,
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
