package incari.features.schematics;

import incari.framework.game.Blocks;
import java.util.Arrays;
import org.jspecify.annotations.Nullable;

/**
 * Compares a placed schematic with the world, a 16-block section at a time, a few sections a tick:
 * which blocks are right, missing, the wrong block, the right block in the wrong state, or there
 * when the schematic wants air. Sections near the player are looked at again every few ticks, so
 * what they build shows at once; the rest in turn, for the counts and the material list. A section
 * whose chunk the client hasn't loaded counts as nothing yet.
 *
 * <p>Game thread. {@link #tick} is a hot path: it allocates only when a section first needs room
 * for its problems.
 */
final class Verifier {

    /** A problem's kind, as {@link Section#issue} packs it. */
    static final int MISSING = 1;

    static final int WRONG_BLOCK = 2;
    static final int WRONG_STATE = 3;
    static final int EXTRA = 4;

    /** Blocks looked at in a tick, at most (a section past it is finished). */
    static final int BUDGET = 8192;

    /** How far round the player, in blocks, is "where they're building": looked at every few ticks. */
    static final int REACH = 6;

    /** Ticks between looks at a section where the player is building. */
    static final int NEAR_TICKS = 4;

    /** Ticks between looks at a section near enough that its problems are drawn. */
    static final int SHOWN_TICKS = 40;

    private final Placement placement;
    private final int[] states;
    private final Blocks blocks;

    private final int minSectionX;
    private final int minSectionY;
    private final int minSectionZ;
    private final int countX;
    private final int countY;
    private final int countZ;
    private final Section[] sections;

    /** Right blocks so far by palette index, over every section looked at. */
    private final int[] rightByIndex;

    /** Scratch for a scan: right blocks by palette index, and which indices it touched. */
    private final int[] scratch;

    private final int[] touched = new int[4096];

    private int right;
    private int missing;
    private int wrongBlocks;
    private int wrongStates;
    private int extra;
    private int looked;
    private long ticks;
    private int cursor;

    /** One 16-block section of the world, as far as the schematic's box reaches into it. */
    static final class Section {

        final int sectionX;
        final int sectionY;
        final int sectionZ;

        /** The tick it was last looked at; -1 before the first. */
        long lookedAt = -1;

        boolean loaded;

        /** Goes up whenever its missing blocks or problems change. */
        long revision;

        private int hash;

        int right;
        int missing;
        int wrongBlocks;
        int wrongStates;
        int extra;

        /** Problems other than missing blocks, while it's near enough to draw them: see {@link #issue}. */
        int @Nullable [] issues;

        int issueCount;

        /** Right blocks by palette index: the indices, and how many of each. */
        int[] indices = new int[0];

        int[] counts = new int[0];
        int used;

        Section(int sectionX, int sectionY, int sectionZ) {
            this.sectionX = sectionX;
            this.sectionY = sectionY;
            this.sectionZ = sectionZ;
        }

        /** Problem {@code i}: its kind in the low three bits, then its block in the section as {@code y << 8 | z << 4 | x}. */
        int issue(int i) {
            int[] list = issues;
            return list == null ? 0 : list[i];
        }
    }

    /**
     * @param states the game's state for each palette index, mirrored and turned as placed;
     *     {@link Blocks#UNKNOWN} for one to leave alone
     */
    Verifier(Placement placement, int[] states, Blocks blocks) {
        this.placement = placement;
        this.states = states;
        this.blocks = blocks;
        minSectionX = placement.minX() >> 4;
        minSectionY = placement.minY() >> 4;
        minSectionZ = placement.minZ() >> 4;
        countX = ((placement.maxX() - 1) >> 4) - minSectionX + 1;
        countY = ((placement.maxY() - 1) >> 4) - minSectionY + 1;
        countZ = ((placement.maxZ() - 1) >> 4) - minSectionZ + 1;
        sections = new Section[countX * countY * countZ];
        for (int sy = 0; sy < countY; sy++) {
            for (int sz = 0; sz < countZ; sz++) {
                for (int sx = 0; sx < countX; sx++) {
                    sections[(sy * countZ + sz) * countX + sx] =
                            new Section(minSectionX + sx, minSectionY + sy, minSectionZ + sz);
                }
            }
        }
        rightByIndex = new int[states.length];
        scratch = new int[states.length];
    }

    /** The section at section coordinates {@code sx, sy, sz}, or null if the box doesn't reach it. */
    @Nullable
    Section section(int sx, int sy, int sz) {
        int ix = sx - minSectionX;
        int iy = sy - minSectionY;
        int iz = sz - minSectionZ;
        if (ix < 0 || iy < 0 || iz < 0 || ix >= countX || iy >= countY || iz >= countZ) {
            return null;
        }
        return sections[(iy * countZ + iz) * countX + ix];
    }

    /**
     * A tick's looking, round the player at {@code x, y, z}: where they build first, then what's
     * within {@code shownRange} blocks (whose problems are drawn), then the rest in turn.
     */
    void tick(int x, int y, int z, int shownRange) {
        ticks++;
        // Where they're building, but never more than half the tick.
        int budget = BUDGET - lookAround(x, y, z, REACH, NEAR_TICKS, BUDGET / 2, shownRange);
        // One section in turn every tick, whatever else is due, so far ones are still counted.
        budget -= inTurn(x, y, z, shownRange, 1);
        if (budget > 0) {
            budget -= lookAround(x, y, z, shownRange, SHOWN_TICKS, budget, shownRange);
        }
        if (budget > 0) {
            inTurn(x, y, z, shownRange, budget);
        }
    }

    /** Looks at sections in turn not looked at lately, until {@code budget} blocks are looked at. */
    private int inTurn(int x, int y, int z, int shownRange, int budget) {
        int spent = 0;
        for (int tries = 0; spent < budget && tries < sections.length; tries++) {
            Section section = sections[cursor];
            cursor = (cursor + 1) % sections.length;
            if (section.lookedAt < 0 || ticks - section.lookedAt > SHOWN_TICKS * 5) {
                spent += look(section, near(section, x, y, z, shownRange));
            }
        }
        return spent;
    }

    /** Looks at the sections within {@code range} of the player not looked at for {@code every} ticks. */
    private int lookAround(int x, int y, int z, int range, int every, int budget, int shownRange) {
        int spent = 0;
        int sx0 = Math.max(minSectionX, (x - range) >> 4);
        int sy0 = Math.max(minSectionY, (y - range) >> 4);
        int sz0 = Math.max(minSectionZ, (z - range) >> 4);
        int sx1 = Math.min(minSectionX + countX - 1, (x + range) >> 4);
        int sy1 = Math.min(minSectionY + countY - 1, (y + range) >> 4);
        int sz1 = Math.min(minSectionZ + countZ - 1, (z + range) >> 4);
        for (int sy = sy0; sy <= sy1 && spent < budget; sy++) {
            for (int sz = sz0; sz <= sz1 && spent < budget; sz++) {
                for (int sx = sx0; sx <= sx1 && spent < budget; sx++) {
                    Section section =
                            sections[((sy - minSectionY) * countZ + (sz - minSectionZ)) * countX + (sx - minSectionX)];
                    boolean shown = near(section, x, y, z, shownRange);
                    boolean stale = section.lookedAt < 0 || ticks - section.lookedAt >= every;
                    // A section come into view needs its problems listed now, not at its next turn.
                    if (stale || (shown && section.loaded && section.issues == null && hasProblems(section))) {
                        spent += look(section, shown);
                    }
                }
            }
        }
        return spent;
    }

    private static boolean hasProblems(Section section) {
        return section.wrongBlocks + section.wrongStates + section.extra > 0;
    }

    /** Whether the section is within {@code range} blocks of {@code x, y, z}, its problems kept to draw. */
    private static boolean near(Section section, int x, int y, int z, int range) {
        int dx = Math.max(0, Math.max((section.sectionX << 4) - x, x - (section.sectionX << 4) - 15));
        int dy = Math.max(0, Math.max((section.sectionY << 4) - y, y - (section.sectionY << 4) - 15));
        int dz = Math.max(0, Math.max((section.sectionZ << 4) - z, z - (section.sectionZ << 4) - 15));
        return (long) dx * dx + (long) dy * dy + (long) dz * dz <= (long) range * range;
    }

    /**
     * Compares one section with the world and updates the counts.
     *
     * @param keepIssues whether to list its problems, to draw them
     * @return how many blocks it looked at
     */
    private int look(Section section, boolean keepIssues) {
        forget(section);
        section.lookedAt = ticks;
        int x0 = Math.max(section.sectionX << 4, placement.minX());
        int y0 = Math.max(section.sectionY << 4, placement.minY());
        int z0 = Math.max(section.sectionZ << 4, placement.minZ());
        int x1 = Math.min((section.sectionX << 4) + 16, placement.maxX());
        int y1 = Math.min((section.sectionY << 4) + 16, placement.maxY());
        int z1 = Math.min((section.sectionZ << 4) + 16, placement.maxZ());
        int[] issues = keepIssues ? section.issues : null;
        if (!keepIssues) {
            section.issues = null;
        }
        int issueCount = 0;
        int used = 0;
        int hash = 1;
        boolean loaded = true;
        int visited = 0;
        scan:
        for (int y = y0; y < y1; y++) {
            for (int z = z0; z < z1; z++) {
                for (int x = x0; x < x1; x++) {
                    visited++;
                    int index = placement.indexAt(x, y, z);
                    if (index == 0) {
                        continue;
                    }
                    int want = states[index];
                    if (want == Blocks.UNKNOWN) {
                        continue;
                    }
                    int have = blocks.at(x, y, z);
                    if (have == Blocks.UNKNOWN) {
                        loaded = false;
                        break scan;
                    }
                    int kind;
                    if (have == want) {
                        kind = 0;
                    } else if (blocks.isAir(want)) {
                        kind = blocks.isAir(have) ? 0 : EXTRA;
                    } else if (blocks.isAir(have)) {
                        kind = MISSING;
                    } else {
                        kind = blocks.block(have) == blocks.block(want) ? WRONG_STATE : WRONG_BLOCK;
                    }
                    if (kind == 0) {
                        if (!blocks.isAir(want)) {
                            section.right++;
                            if (scratch[index]++ == 0) {
                                touched[used++] = index;
                            }
                        }
                        continue;
                    }
                    int issue = ((y & 15) << 8 | (z & 15) << 4 | (x & 15)) << 3 | kind;
                    hash = hash * 31 + issue;
                    switch (kind) {
                        case MISSING -> section.missing++;
                        case WRONG_BLOCK -> section.wrongBlocks++;
                        case WRONG_STATE -> section.wrongStates++;
                        default -> section.extra++;
                    }
                    if (keepIssues && kind != MISSING) {
                        if (issues == null) {
                            issues = new int[64];
                        } else if (issueCount == issues.length) {
                            issues = Arrays.copyOf(issues, Math.min(4096, issues.length * 2));
                        }
                        issues[issueCount++] = issue;
                    }
                }
            }
        }
        if (!loaded) {
            for (int i = 0; i < used; i++) {
                scratch[touched[i]] = 0;
            }
            section.right = section.missing = section.wrongBlocks = section.wrongStates = section.extra = 0;
            used = 0;
            issueCount = 0;
            hash = 0;
        }
        section.loaded = loaded;
        if (keepIssues) {
            section.issues = issues;
        }
        section.issueCount = issueCount;
        if (section.indices.length < used) {
            section.indices = new int[Math.max(used, section.indices.length * 2)];
            section.counts = new int[section.indices.length];
        }
        for (int i = 0; i < used; i++) {
            int index = touched[i];
            section.indices[i] = index;
            section.counts[i] = scratch[index];
            scratch[index] = 0;
        }
        section.used = used;
        if (hash != section.hash) {
            section.hash = hash;
            section.revision++;
        }
        remember(section);
        return Math.max(1, visited);
    }

    /** Takes the section's counts out of the totals, before it's looked at again. */
    private void forget(Section section) {
        if (section.lookedAt < 0) {
            return;
        }
        right -= section.right;
        missing -= section.missing;
        wrongBlocks -= section.wrongBlocks;
        wrongStates -= section.wrongStates;
        extra -= section.extra;
        looked--;
        for (int i = 0; i < section.used; i++) {
            rightByIndex[section.indices[i]] -= section.counts[i];
        }
        section.right = section.missing = section.wrongBlocks = section.wrongStates = section.extra = 0;
    }

    private void remember(Section section) {
        right += section.right;
        missing += section.missing;
        wrongBlocks += section.wrongBlocks;
        wrongStates += section.wrongStates;
        extra += section.extra;
        looked++;
        for (int i = 0; i < section.used; i++) {
            rightByIndex[section.indices[i]] += section.counts[i];
        }
    }

    /** Blocks found right, air left out. */
    int right() {
        return right;
    }

    int missing() {
        return missing;
    }

    int wrongBlocks() {
        return wrongBlocks;
    }

    int wrongStates() {
        return wrongStates;
    }

    int extra() {
        return extra;
    }

    /** Right blocks of palette index {@code index}, over the sections looked at. */
    int rightOf(int index) {
        return rightByIndex[index];
    }

    /** How many sections there are. */
    int sectionCount() {
        return sections.length;
    }

    /** Whether every section has been looked at once. */
    boolean lookedAtAll() {
        return looked == sections.length;
    }
}
