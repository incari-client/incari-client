package incari.client.net;

import java.util.concurrent.TimeUnit;

/**
 * The time left before an automatic reconnect, in whole seconds rounded up, so the label reads
 * "5" for the first second and fires as it would show "0". Plain arithmetic, kept apart to test.
 */
final class ReconnectCountdown {

    /** Seconds between being kicked and joining again. */
    static final int SECONDS = 5;

    private static final long NANOS_PER_SECOND = TimeUnit.SECONDS.toNanos(1);

    private long deadlineNanos;

    /** Starts counting {@link #SECONDS} down from {@code nowNanos}. */
    void start(long nowNanos) {
        deadlineNanos = nowNanos + SECONDS * NANOS_PER_SECOND;
    }

    /** Whole seconds left at {@code nowNanos}, rounded up; 0 once it's time. */
    int secondsLeft(long nowNanos) {
        long left = deadlineNanos - nowNanos;
        return left <= 0 ? 0 : (int) ((left + NANOS_PER_SECOND - 1) / NANOS_PER_SECOND);
    }
}
