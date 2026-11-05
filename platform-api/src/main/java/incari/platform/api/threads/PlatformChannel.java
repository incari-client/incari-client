package incari.platform.api.threads;

import org.jspecify.annotations.Nullable;

/**
 * Messages between the game's program and a second copy of it running apart, in a worker
 * ({@link PlatformThreads#startGameWorker}): the page and singleplayer's server (ADR 0039). Each
 * message is a kind (a number the two sides agree on) and bytes. Messages arrive in the order they
 * were sent.
 *
 * <p>Both ends belong to one thread each at a time: {@link #receive} waits, so it runs on a thread
 * that may (never the render thread).
 */
public interface PlatformChannel {

    /** A message: its kind and its bytes, which the receiver owns. */
    final class Message {
        private final int kind;
        private final byte[] bytes;

        public Message(int kind, byte[] bytes) {
            this.kind = kind;
            this.bytes = bytes;
        }

        public int kind() {
            return kind;
        }

        /** The message's bytes, the receiver's to keep or change. */
        public byte[] bytes() {
            return bytes;
        }
    }

    /**
     * Sends a message without waiting. The bytes are copied (or handed over) before this returns,
     * so the caller may reuse the array.
     */
    void send(int kind, byte[] bytes, int offset, int length);

    /** Sends a message with no bytes. */
    default void send(int kind) {
        send(kind, new byte[0], 0, 0);
    }

    /**
     * Waits for the next message, or returns null once the channel is closed (by either end, or
     * because the other program stopped).
     */
    @Nullable
    Message receive();

    /** The next message if one has arrived, without waiting; null if none has. */
    @Nullable
    Message poll();

    /**
     * Asks the platform to send an empty message of {@code kind} on its own whenever the page is
     * about to be hidden, reloaded or closed: when the game's thread may never run again, so it
     * can't send it itself (singleplayer's server saves on it). From the side that started the
     * worker; the desktop, which has no page, ignores it. Once per channel; ends with it.
     */
    default void sendWhenLeaving(int kind) {}

    /** Closes the channel; the other end's {@link #receive} returns null. Ends the worker it started. */
    void close();
}
