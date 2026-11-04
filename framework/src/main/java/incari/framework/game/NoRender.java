package incari.framework.game;

/**
 * What the client leaves out of the world it draws ({@link Game#overrideNoRender}), on this client
 * only: what's hidden is still there, to pick up, hit and collide with. Mostly for frames: a bone
 * meal farm's items and sparkles can take a Chromebook to no frames at all.
 *
 * @param items how items on the ground are drawn
 * @param oneCopy a stack on the ground drawn as one item, not as the game's pile of up to five
 * @param orbs experience orbs hidden
 * @param fallingBlocks falling sand, gravel, anvils and the rest hidden
 * @param fireworks firework rockets and their bursts hidden
 * @param growth the green sparkles of bone meal and growing crops hidden
 * @param breaking the bits of blocks being broken, and the dust of running and landing, hidden
 * @param explosions explosions' smoke clouds hidden
 * @param allParticles every particle hidden
 */
public record NoRender(
        Items items,
        boolean oneCopy,
        boolean orbs,
        boolean fallingBlocks,
        boolean fireworks,
        boolean growth,
        boolean breaking,
        boolean explosions,
        boolean allParticles) {

    /** How items on the ground are drawn. */
    public enum Items {
        /** As the game draws them, bobbing and spinning. */
        SHOWN("Shown"),
        /** Lying still: no bobbing or spinning. */
        STILL("Still"),
        /** Not drawn at all. */
        HIDDEN("Hidden");

        private final String label;

        Items(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }
}
