package incari.net.msa;

/** What {@code sessionserver} said to a join: the server then checks it said yes. */
public enum JoinResult {
    /** Joined: the server will find this account in the session server. */
    OK,
    /** The Minecraft token was refused: sign in again. */
    INVALID_SESSION,
    /** The account may not play online (a child account, or multiplayer turned off). */
    INSUFFICIENT_PRIVILEGES,
    /** The account is banned from multiplayer. */
    BANNED,
    /** The session server couldn't be reached or answered something else. */
    UNAVAILABLE
}
