package incari.classlib.substitutes.java.util.concurrent;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

/** The default methods, over a map that only provides the abstract ones (as Guava's do). */
class ConcurrentMapTest {

    private static final class Plain<K, V> extends AbstractMap<K, V> implements TConcurrentMap<K, V> {
        private final Map<K, V> map = new HashMap<>();

        @Override
        public Set<Map.Entry<K, V>> entrySet() {
            return map.entrySet();
        }

        @Override
        public @Nullable V put(K key, V value) {
            return map.put(key, value);
        }

        @Override
        public @Nullable V putIfAbsent(K key, V value) {
            return map.putIfAbsent(key, value);
        }

        @Override
        public boolean remove(Object key, @Nullable Object value) {
            return map.remove(key, value);
        }

        @Override
        public boolean replace(K key, V oldValue, V newValue) {
            return map.replace(key, oldValue, newValue);
        }

        @Override
        public @Nullable V replace(K key, V value) {
            return map.replace(key, value);
        }

        @Override
        public boolean equals(@Nullable Object o) {
            return super.equals(o);
        }

        @Override
        public int hashCode() {
            return Objects.hashCode(map);
        }
    }

    @Test
    void computeIfAbsentAnswersTheValue() {
        Plain<String, String> map = new Plain<>();
        assertThat(map.computeIfAbsent("a", k -> "made " + k)).isEqualTo("made a");
        assertThat(map.computeIfAbsent("a", k -> "again")).isEqualTo("made a");
        assertThat(map.computeIfAbsent("b", k -> null)).isNull();
        assertThat(map).containsOnlyKeys("a");
    }

    @Test
    void computeMergeAndPresentFollowTheContract() {
        Plain<String, Integer> map = new Plain<>();
        assertThat(map.merge("x", 1, Integer::sum)).isEqualTo(1);
        assertThat(map.merge("x", 2, Integer::sum)).isEqualTo(3);
        assertThat(map.compute("x", (k, v) -> v == null ? 0 : v * 10)).isEqualTo(30);
        assertThat(map.computeIfPresent("x", (k, v) -> null)).isNull();
        assertThat(map).isEmpty();
        assertThat(map.compute("y", (k, v) -> 5)).isEqualTo(5);
        assertThat(map.getOrDefault("z", 9)).isEqualTo(9);
    }
}
