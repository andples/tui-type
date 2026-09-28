//! Every state change in the app is an `Action`. Key events, commands and
//! (later) remote events all reduce to this enum and go through
//! `App::dispatch`, which keeps the UI layer purely presentational.

#[derive(Debug, Clone, PartialEq, Eq)]
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

    Quit,
    Nop,
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
