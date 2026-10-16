package incari.framework.event;

/**
 * The mouse turning the view, as {@link Events#TURN} hands it to listeners, in degrees after the
 * player's sensitivity and inversion. A listener that takes the turn ({@link #consume()}) keeps it
 * from the player: Freelook turns the camera instead. One that {@link #scale}s it turns the player
 * less: Zoom, while zoomed in. One object, reused.
 */
public final class Turn {

    private float yaw;
    private float pitch;
    private boolean consumed;
    private boolean scaled;

    /** The game's side: this turn, before the listeners run. */
    public void reset(float yaw, float pitch) {
        this.yaw = yaw;
        this.pitch = pitch;
        this.consumed = false;
        this.scaled = false;
    }

    /** Degrees to the right. */
    public float yaw() {
        return yaw;
    }

    /** Degrees down. */
    public float pitch() {
        return pitch;
    }

    /** Keeps the turn from the player. */
    public void consume() {
        consumed = true;
    }

    /** Whether a listener took it. */
    public boolean consumed() {
        return consumed;
    }

    /** Turns the player {@code factor} times as far (0.25: a quarter as far). */
    public void scale(float factor) {
        yaw *= factor;
        pitch *= factor;
        scaled = true;
    }

    /** Whether a listener scaled it: the player turns by {@link #yaw()} and {@link #pitch()} as they are now. */
    public boolean scaled() {
        return scaled;
    }
}
