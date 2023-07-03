// This chould be kept in sync with: src/MessagesToFe.ts

// Connecting sequence
pub const CONNECTING_SERVER : &str = "CONNECTING_SERVER";
pub const SERVER_CONNECTED_WAITING_USER : &str = "SERVER_CONNECTED_WAITING_USER";
pub const USER_CONNECTING : &str = "USER_CONNECTING";
pub const USER_CONNECTED : &str = "USER_CONNECTED";

// During normal use
pub const CONTROLLING_STARTED : &str = "CONTROLLING_STARTED";
pub const CONTROLLING_STOPPED : &str = "CONTROLLING_STOPPED";

// User leaves
pub const USER_DISCONNECTED : &str = "USER_DISCONNECTED";

// Timeout if SERVER_CONNECTED_WAITING_USER too long
pub const SERVER_DISCONNECTED : &str = "SERVER_DISCONNECTED";
