// This chould be kept in sync with: src-tauri/src/main_process/messages_to_fe.rs

// Connecting sequence
export const CONNECTING_SERVER : string = "CONNECTING_SERVER";
export const SERVER_CONNECTED_WAITING_USER : string = "SERVER_CONNECTED_WAITING_USER";
export const USER_CONNECTING : string = "USER_CONNECTING";
export const USER_CONNECTED : string = "USER_CONNECTED";

// During normal use
export const CONTROLLING_STARTED : string = "CONTROLLING_STARTED";
export const CONTROLLING_STOPPED : string = "CONTROLLING_STOPPED";

// User leaves
export const USER_DISCONNECTED : string = "USER_DISCONNECTED";

// Timeout if SERVER_CONNECTED_WAITING_USER too long
export const SERVER_DISCONNECTED : string = "SERVER_DISCONNECTED";
