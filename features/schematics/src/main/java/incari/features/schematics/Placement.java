package incari.features.schematics;

import incari.features.schematics.format.Schematic;
import incari.framework.game.Mirror;

/**
 * Where a schematic stands in the world: its box's lowest corner, and how it's mirrored and then
 * turned. Turning and mirroring keep that corner where it is, so the box only changes shape: the
 * player sees it turn in place rather than swing about a point. Turns are clockwise seen from above,
 * as the game's own rotations: what faced north faces east after one.
 */
final class Placement {

    final Schematic schematic;
    private int x;
    private int y;
    private int z;
    private Mirror mirror = Mirror.NONE;
    private int turns;

    Placement(Schematic schematic, int x, int y, int z) {
        this.schematic = schematic;
        moveTo(x, y, z);
    }

    /** Puts the box's lowest corner at {@code x, y, z}. */
    void moveTo(int x, int y, int z) {
        this.x = x;
        this.y = y;
        this.z = z;
    }

    /** Moves it by this many blocks. */
    void moveBy(int dx, int dy, int dz) {
        moveTo(x + dx, y + dy, z + dz);
    }

    /** Turns it {@code quarterTurns} more clockwise (negative: anticlockwise). */
    void turn(int quarterTurns) {
        turns = Math.floorMod(turns + quarterTurns, 4);
    }

    void mirror(Mirror mirror) {
        this.mirror = mirror;
    }

    Mirror mirror() {
        return mirror;
    }

    /** Clockwise quarter turns, 0 to 3. */
    int turns() {
        return turns;
    }

    int minX() {
        return x;
    }

    int minY() {
        return y;
    }

    int minZ() {
        return z;
    }

    /** The placed box's size along x: the schematic's z after an odd number of turns. */
    int sizeX() {
        return (turns & 1) == 0 ? schematic.sizeX() : schematic.sizeZ();
    }

    int sizeY() {
        return schematic.sizeY();
    }

    int sizeZ() {
        return (turns & 1) == 0 ? schematic.sizeZ() : schematic.sizeX();
    }

    /** One past the box's last block: x. */
    int maxX() {
        return x + sizeX();
    }

    int maxY() {
        return y + sizeY();
    }

    int maxZ() {
        return z + sizeZ();
    }

    /** Whether the world block {@code wx, wy, wz} is in the box. */
    boolean contains(int wx, int wy, int wz) {
        return wx >= x && wy >= y && wz >= z && wx < maxX() && wy < maxY() && wz < maxZ();
    }

    /**
     * The schematic's palette index for the world block {@code wx, wy, wz}: 0 (nothing wanted)
     * outside the box. Hot path: arithmetic only.
     */
    int indexAt(int wx, int wy, int wz) {
        int px = wx - x;
        int py = wy - y;
        int pz = wz - z;
        int sx = schematic.sizeX();
        int sz = schematic.sizeZ();
        if (py < 0 || py >= schematic.sizeY() || px < 0 || pz < 0) {
            return 0;
        }
        // Undo the turn, then the mirror: placed box -> mirrored schematic -> schematic.
        int mx;
        int mz;
        switch (turns) {
            case 1 -> {
                if (px >= sz || pz >= sx) {
                    return 0;
                }
                mx = pz;
                mz = sz - 1 - px;
            }
            case 2 -> {
                if (px >= sx || pz >= sz) {
                    return 0;
                }
                mx = sx - 1 - px;
                mz = sz - 1 - pz;
            }
            case 3 -> {
                if (px >= sz || pz >= sx) {
                    return 0;
                }
                mx = sx - 1 - pz;
                mz = px;
            }
            default -> {
                if (px >= sx || pz >= sz) {
                    return 0;
                }
                mx = px;
                mz = pz;
            }
        }
        int lx = mirror == Mirror.X ? sx - 1 - mx : mx;
        int lz = mirror == Mirror.Z ? sz - 1 - mz : mz;
        return schematic.at(lx, py, lz);
    }

    /**
     * Where the schematic's own block {@code lx, ly, lz} lands in the world, into {@code out} as x,
     * y, z: mirrored, then turned. The inverse of {@link #indexAt}'s lookup.
     */
    void toWorld(int lx, int ly, int lz, int[] out) {
        int sx = schematic.sizeX();
        int sz = schematic.sizeZ();
        int mx = mirror == Mirror.X ? sx - 1 - lx : lx;
        int mz = mirror == Mirror.Z ? sz - 1 - lz : lz;
        int px;
        int pz;
        switch (turns) {
            case 1 -> {
                px = sz - 1 - mz;
                pz = mx;
            }
            case 2 -> {
                px = sx - 1 - mx;
                pz = sz - 1 - mz;
            }
            case 3 -> {
                px = mz;
                pz = sx - 1 - mx;
            }
            default -> {
                px = mx;
                pz = mz;
            }
        }
        out[0] = x + px;
        out[1] = y + ly;
        out[2] = z + pz;
    }
}
