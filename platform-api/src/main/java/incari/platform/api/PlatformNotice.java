package incari.platform.api;

/**
 * Something the platform found that the player must hear about, because it changes what happens to
 * their game: chiefly, that what they do won't be saved. Delivered through
 * {@code PlatformEventListener.notice} on the polling thread; the game words it for the player.
 */
public enum PlatformNotice {

    /** The game is open in another tab or window, which keeps the files: nothing here is saved. */
    SAVES_HELD_ELSEWHERE,

    /** The browser gives the page no storage (a private window, storage blocked): nothing is saved. */
    SAVES_UNAVAILABLE,

    /**
     * Saving failed (the disk is full, or storage refused the write). What failed is tried again a
     * few seconds later; reported once until saving works again.
     */
    SAVE_FAILED
}
