package incari.client.singleplayer;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.util.UUID;
import org.jspecify.annotations.Nullable;

/**
 * What the page and singleplayer's server in a worker say to each other (ADR 0039): the kinds of
 * message on their {@code PlatformChannel}, and the start message's contents. Packets are the
 * game's own bytes, each with its length in front as over TCP, a flush of them per message
 * (ADR 0041).
 */
final class WorldMessages {

    // --- From the page ------------------------------------------------------------------------

    /** Open a world: {@link Start}. */
    static final int START = 1;
    /** Packets for the server: a flush's worth of the stream. */
    static final int PACKET_TO_SERVER = 2;
    /** Whether the game is paused: one byte. */
    static final int PAUSED = 3;
    /** View and simulation distance, in chunks: two ints. */
    static final int DISTANCES = 4;
    /** Save and stop; the worker answers with {@link #STOPPED}. */
    static final int STOP = 5;
    /** The client closed its connection. */
    static final int CLIENT_CLOSED = 6;
    /** The world's icon, a PNG the client took. */
    static final int ICON = 7;
    /** World Options changed a host setting: one of the {@code HOST_} settings, then its value (two ints). */
    static final int HOST_CHANGE = 8;

    /** Whether the owner may use commands: 1 or 0. */
    static final int HOST_ALLOW_COMMANDS = 1;
    /** The world's default game mode: its id. */
    static final int HOST_WORLD_GAME_MODE = 2;
    /** The owner's own game mode: its id. */
    static final int HOST_PERSONAL_GAME_MODE = 3;
    /** Whether players' game modes follow the world's: 1 or 0. */
    static final int HOST_FORCE_GAME_MODE = 4;
    /** Whether guests may use commands: 1 or 0. */
    static final int HOST_GUEST_COMMANDS = 5;

    /**
     * Save now, and play on: the page is being hidden, reloaded or closed. Sent by the page's own
     * listeners, not the game ({@code PlatformChannel.sendWhenLeaving}), as no game frame may come.
     */
    static final int SAVE = 13;

    // --- A shared world's guests (ADR 0062): each message starts with the guest's number ---------

    /** Whether the world is shared now (guests may join): one byte, 1 or 0. */
    static final int SHARED = 9;
    /** A guest's data channel opened: the guest's number. */
    static final int GUEST_OPEN = 10;
    /** A message from a guest: its number, then the data channel's message. */
    static final int GUEST_MESSAGE = 11;
    /** A guest's data channel closed: its number. */
    static final int GUEST_CLOSED = 12;

    // --- From the worker ------------------------------------------------------------------------

    /** The world's load: a stage's name, then chunks done and in all. */
    static final int LOAD_PROGRESS = 20;
    /** The server is running and takes a connection: one byte, 1 if the world has an icon. */
    static final int READY = 21;
    /** Packets for the client: a flush's worth of the stream. */
    static final int PACKET_TO_CLIENT = 22;
    /** Changes to the world's files for the page's store. */
    static final int FILE_CHANGES = 23;
    /** Stopped and saved: the world's files, handed back. */
    static final int STOPPED = 24;
    /** The server failed: a message for the player. */
    static final int FAILED = 25;
    /** The server closed the client's connection. */
    static final int SERVER_CLOSED = 26;
    /** Server tick time for the debug screen: average milliseconds a tick, as a float. */
    static final int TICK_TIME = 27;
    /**
     * The host settings World Options shows: the default game mode's id, then allow commands, force
     * game mode and guest commands as 1 or 0 (four ints).
     */
    static final int HOST_STATE = 28;
    /** A message for a guest: its number, then the data channel's message. */
    static final int GUEST_SEND = 29;
    /** The server closed a guest's connection: its number. */
    static final int GUEST_END = 30;

    private WorldMessages() {}

    /**
     * What opening a world takes.
     *
     * @param levelId the world's folder name in {@code saves}
     * @param hostName the player's name, the world's owner
     * @param hostId the player's id
     * @param viewDistance the client's render distance, in chunks
     * @param simulationDistance the client's simulation distance, in chunks
     * @param files the world's files, handed over by the page's storage
     */
    @SuppressWarnings("ArrayRecordComponent") // Carries bytes; never compared or hashed.
    record Start(String levelId, String hostName, UUID hostId, int viewDistance, int simulationDistance, byte[] files) {

        byte[] encode() {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(files.length + 256);
            try (DataOutputStream out = new DataOutputStream(bytes)) {
                out.writeUTF(levelId);
                out.writeUTF(hostName);
                out.writeLong(hostId.getMostSignificantBits());
                out.writeLong(hostId.getLeastSignificantBits());
                out.writeInt(viewDistance);
                out.writeInt(simulationDistance);
                writeBytes(out, files);
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
            return bytes.toByteArray();
        }

        static Start decode(byte[] message) {
            try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(message))) {
                String levelId = in.readUTF();
                String hostName = in.readUTF();
                UUID hostId = new UUID(in.readLong(), in.readLong());
                int viewDistance = in.readInt();
                int simulationDistance = in.readInt();
                byte[] files = readBytes(in);
                return new Start(
                        levelId,
                        hostName,
                        hostId,
                        viewDistance,
                        simulationDistance,
                        files == null ? new byte[0] : files);
            } catch (IOException e) {
                throw new UncheckedIOException("A damaged start message", e);
            }
        }
    }

    /** A stage of the world's load and how far it is, for the page's loading screen. */
    record Progress(String stage, int done, int total) {

        byte[] encode() {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            try (DataOutputStream out = new DataOutputStream(bytes)) {
                out.writeUTF(stage);
                out.writeInt(done);
                out.writeInt(total);
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
            return bytes.toByteArray();
        }

        static Progress decode(byte[] message) {
            try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(message))) {
                return new Progress(in.readUTF(), in.readInt(), in.readInt());
            } catch (IOException e) {
                throw new UncheckedIOException("A damaged progress message", e);
            }
        }
    }

    static byte[] ints(int... values) {
        byte[] bytes = new byte[values.length * 4];
        for (int i = 0; i < values.length; i++) {
            int value = values[i];
            bytes[i * 4] = (byte) (value >>> 24);
            bytes[i * 4 + 1] = (byte) (value >>> 16);
            bytes[i * 4 + 2] = (byte) (value >>> 8);
            bytes[i * 4 + 3] = (byte) value;
        }
        return bytes;
    }

    /** Writes {@code value} big-endian at byte {@code at}. */
    static void putInt(byte[] bytes, int at, int value) {
        bytes[at] = (byte) (value >>> 24);
        bytes[at + 1] = (byte) (value >>> 16);
        bytes[at + 2] = (byte) (value >>> 8);
        bytes[at + 3] = (byte) value;
    }

    static int intAt(byte[] bytes, int index) {
        int at = index * 4;
        return ((bytes[at] & 0xFF) << 24)
                | ((bytes[at + 1] & 0xFF) << 16)
                | ((bytes[at + 2] & 0xFF) << 8)
                | (bytes[at + 3] & 0xFF);
    }

    private static void writeBytes(DataOutputStream out, byte @Nullable [] bytes) throws IOException {
        out.writeInt(bytes == null ? -1 : bytes.length);
        if (bytes != null) {
            out.write(bytes);
        }
    }

    private static byte @Nullable [] readBytes(DataInputStream in) throws IOException {
        int length = in.readInt();
        if (length < 0) {
            return null;
        }
        byte[] bytes = new byte[length];
        in.readFully(bytes);
        return bytes;
    }
}
