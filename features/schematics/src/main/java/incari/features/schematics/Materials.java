package incari.features.schematics;

import incari.features.schematics.format.BlockSpec;
import incari.features.schematics.format.Schematic;
import incari.framework.game.Blocks;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * What a schematic takes to build, as items: how many of each in all, and how many are still
 * needed once the right blocks already in the world are taken off. A door is one item for its two
 * blocks, a double slab two; blocks no item places (water, fire) are left out. Game thread; {@link
 * #refresh} allocates nothing.
 */
final class Materials {

    /** Properties that say how many items a block holds. */
    private static final String[] COUNTED = {"candles", "pickles", "eggs", "layers", "flower_amount", "segment_amount"};

    private final String[] items;
    private final int[] totals;
    private final int[] needed;
    private final int[] order;
    private final int[] itemOf;
    private final int[] costOf;
    private int itemsNeeded;
    private int totalNeeded;

    /**
     * @param states the game's state for each palette index, as the schematic has it (unturned);
     *     {@link Blocks#UNKNOWN} for one the game hasn't
     */
    Materials(Schematic schematic, int[] states, Blocks blocks) {
        List<BlockSpec> palette = schematic.palette();
        itemOf = new int[palette.size()];
        costOf = new int[palette.size()];
        Map<String, Integer> byId = new HashMap<>();
        List<String> ids = new ArrayList<>();
        List<Integer> sums = new ArrayList<>();
        for (int i = 0; i < itemOf.length; i++) {
            itemOf[i] = -1;
            BlockSpec spec = palette.get(i);
            int state = states[i];
            if (spec.isNothing() || spec.isAir() || state == Blocks.UNKNOWN) {
                continue;
            }
            String id = blocks.itemId(state);
            int cost = cost(spec);
            if (id.isEmpty() || cost == 0) {
                continue;
            }
            Integer item = byId.get(id);
            if (item == null) {
                item = ids.size();
                ids.add(id);
                sums.add(0);
                byId.put(id, item);
            }
            itemOf[i] = item;
            costOf[i] = cost;
            sums.set(item, sums.get(item) + schematic.count(i) * cost);
        }
        items = ids.toArray(new String[0]);
        totals = new int[items.length];
        for (int i = 0; i < totals.length; i++) {
            totals[i] = sums.get(i);
        }
        needed = totals.clone();
        order = new int[items.length];
        sort();
    }

    /**
     * How many items one block in state {@code spec} takes: none for the second half of a door, bed
     * or tall plant, two for a double slab, as many as it holds of candles, pickles, eggs, snow
     * layers and petals.
     */
    static int cost(BlockSpec spec) {
        if (spec.property("half", "").equals("upper")
                || spec.property("part", "").equals("head")) {
            return 0;
        }
        if (spec.name().endsWith("_slab") && spec.property("type", "").equals("double")) {
            return 2;
        }
        for (String key : COUNTED) {
            String value = spec.properties().get(key);
            if (value != null) {
                try {
                    return Math.max(1, Integer.parseInt(value));
                } catch (NumberFormatException e) {
                    return 1;
                }
            }
        }
        return 1;
    }

    /** Takes what's already built off the totals, and sorts by what's needed most. */
    void refresh(Verifier verifier) {
        System.arraycopy(totals, 0, needed, 0, totals.length);
        for (int i = 0; i < itemOf.length; i++) {
            int item = itemOf[i];
            if (item >= 0) {
                needed[item] -= verifier.rightOf(i) * costOf[i];
            }
        }
        sort();
    }

    /** {@link #order} by most needed first, then by id; and the counts of what's left. */
    private void sort() {
        int kinds = 0;
        int sum = 0;
        for (int i = 0; i < order.length; i++) {
            order[i] = i;
            if (needed[i] > 0) {
                kinds++;
                sum += needed[i];
            }
        }
        // Insertion sort: a few hundred items at most, and no boxing.
        for (int i = 1; i < order.length; i++) {
            int item = order[i];
            int j = i - 1;
            while (j >= 0 && before(item, order[j])) {
                order[j + 1] = order[j];
                j--;
            }
            order[j + 1] = item;
        }
        itemsNeeded = kinds;
        totalNeeded = sum;
    }

    private boolean before(int a, int b) {
        if (needed[a] != needed[b]) {
            return needed[a] > needed[b];
        }
        return items[a].compareTo(items[b]) < 0;
    }

    /** How many kinds of item there are. */
    int kinds() {
        return items.length;
    }

    /** The {@code rank}th item, most needed first: its index for the getters. */
    int ranked(int rank) {
        return order[rank];
    }

    String id(int item) {
        return items[item];
    }

    int total(int item) {
        return totals[item];
    }

    /** What's still needed of it: never below 0. */
    int needed(int item) {
        return Math.max(0, needed[item]);
    }

    /** How many kinds still need placing. */
    int kindsNeeded() {
        return itemsNeeded;
    }

    /** How many items still need placing, all kinds together. */
    int totalNeeded() {
        return totalNeeded;
    }
}
