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
    Quit,
    Nop,
}
