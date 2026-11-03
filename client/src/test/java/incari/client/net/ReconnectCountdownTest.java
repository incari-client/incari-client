package incari.client.net;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class ReconnectCountdownTest {

    private static final long SECOND = TimeUnit.SECONDS.toNanos(1);

    @Test
    void countsWholeSecondsDownRoundingUp() {
        ReconnectCountdown countdown = new ReconnectCountdown();
        countdown.start(1_000);
        assertThat(countdown.secondsLeft(1_000)).isEqualTo(ReconnectCountdown.SECONDS);
        assertThat(countdown.secondsLeft(1_000 + SECOND / 2)).isEqualTo(ReconnectCountdown.SECONDS);
        assertThat(countdown.secondsLeft(1_000 + SECOND)).isEqualTo(ReconnectCountdown.SECONDS - 1);
        assertThat(countdown.secondsLeft(1_000 + ReconnectCountdown.SECONDS * SECOND - 1))
                .isEqualTo(1);
    }

    @Test
    void reachesZeroAndStaysThere() {
        ReconnectCountdown countdown = new ReconnectCountdown();
        countdown.start(0);
        assertThat(countdown.secondsLeft(ReconnectCountdown.SECONDS * SECOND)).isZero();
        assertThat(countdown.secondsLeft(ReconnectCountdown.SECONDS * SECOND * 10))
                .isZero();
    }

    @Test
    void startingAgainGivesAFullCountdown() {
        ReconnectCountdown countdown = new ReconnectCountdown();
        countdown.start(0);
        countdown.start(4 * SECOND);
        assertThat(countdown.secondsLeft(4 * SECOND)).isEqualTo(ReconnectCountdown.SECONDS);
    }
}
