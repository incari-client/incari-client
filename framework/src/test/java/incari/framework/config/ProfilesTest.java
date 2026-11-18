package incari.framework.config;

import static org.assertj.core.api.Assertions.assertThat;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import incari.framework.Category;
import incari.framework.Module;
import incari.framework.setting.ColorSetting;
import incari.framework.setting.IntSetting;
import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class ProfilesTest {

    static final class Sample extends Module {

        final IntSetting amount = settings().integer("amount", "Amount", 0, 10, 3);
        final ColorSetting color = settings().color("color", "Color", 0x112233);

        Sample() {
            super("sample", "Sample", "For tests.", Category.UTILITY);
        }
    }

    @Test
    void aNewProfileStartsAsACopyAndSwitchingKeepsEach() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        sample.amount.set(5);

        assertThat(game.system().createProfile("PvP")).isTrue();
        assertThat(game.system().activeProfile()).isEqualTo("PvP");
        assertThat(sample.amount.get()).isEqualTo(5);
        sample.amount.set(9);
        sample.setEnabled(true);

        assertThat(game.system().switchProfile("Default")).isTrue();
        assertThat(sample.amount.get()).isEqualTo(5);
        assertThat(sample.isEnabled()).isFalse();
        assertThat(game.system().switchProfile("pvp")).isTrue();
        assertThat(sample.amount.get()).isEqualTo(9);
        assertThat(sample.isEnabled()).isTrue();
        assertThat(game.system().profiles()).containsExactly("Default", "PvP");
    }

    @Test
    void namesAreCheckedAndUniqueWithoutCase() {
        TestGame game = new TestGame(new Sample());
        assertThat(game.system().createProfile("default")).isFalse();
        assertThat(game.system().createProfile("../escape")).isFalse();
        assertThat(game.system().createProfile(" padded")).isFalse();
        assertThat(game.system().createProfile("")).isFalse();
        assertThat(game.system().createProfile("Building 2")).isTrue();
        assertThat(Profiles.validName("A-b_c 9")).isTrue();
        assertThat(Profiles.validName("café")).isFalse();
        assertThat(Profiles.validName("1.8 PvP")).isTrue();
        assertThat(Profiles.validName("..")).isFalse();
        assertThat(Profiles.validName("a/b")).isFalse();
    }

    @Test
    void deletingTheActiveProfileMovesToTheFirstAndTheLastStays() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.system().createProfile("PvP");
        sample.amount.set(1);
        game.system().flush();

        assertThat(game.system().deleteProfile("PvP")).isTrue();
        assertThat(game.system().activeProfile()).isEqualTo("Default");
        assertThat(sample.amount.get()).isEqualTo(3);
        assertThat(game.files).doesNotContainKey("profiles/PvP.json");
        assertThat(game.system().deleteProfile("Default")).isFalse();
    }

    @Test
    void theActiveProfileIsRememberedAcrossSessions() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.system().createProfile("PvP");
        sample.amount.set(7);
        game.system().flush();

        Sample again = new Sample();
        TestGame next = new TestGame(again);
        next.files.putAll(game.files);
        next.system().load();

        assertThat(next.system().activeProfile()).isEqualTo("PvP");
        assertThat(again.amount.get()).isEqualTo(7);
    }

    @Test
    void modulesThisBuildLacksAreKeptAndBadValuesFallBackToDefaults() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.files.put("profiles.json", "{\"active\": \"Default\", \"profiles\": [\"Default\"]}");
        game.files.put("profiles/Default.json", """
                {"format": 1, "modules": {
                  "sample": {"enabled": true, "settings": {"amount": "lots", "color": "#AABBCC"}},
                  "gone": {"enabled": true, "settings": {"x": 1}}
                }}
                """);
        game.system().load();

        assertThat(sample.isEnabled()).isTrue();
        assertThat(sample.amount.get()).isEqualTo(3);
        assertThat(sample.color.get()).isEqualTo(0xFFAABBCC);

        sample.amount.set(4);
        game.system().flush();
        assertThat(game.files.get("profiles/Default.json")).contains("\"gone\"", "\"x\": 1", "\"amount\": 4");
    }

    @Test
    void aBrokenFileIsReportedAndReadAsDefaults() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.files.put("profiles/Default.json", "{not json");
        sample.amount.set(8);

        game.system().load();

        assertThat(sample.amount.get()).isEqualTo(3);
        assertThat(game.warnings).anyMatch(line -> line.contains("profiles/Default.json"));
        // Not overwritten until something changes.
        assertThat(game.files.get("profiles/Default.json")).isEqualTo("{not json");
    }

    @Test
    void colorsAreWrittenAsHex() {
        assertThat(ProfileCodec.colorText(0xFF00AA11)).isEqualTo("#FF00AA11");
        assertThat(ProfileCodec.colorText(0x0000000F)).isEqualTo("#0000000F");
        assertThat(ProfileCodec.parseColor("#00AA11")).isEqualTo(0xFF00AA11);
        assertThat(ProfileCodec.parseColor("#80000000")).isEqualTo(0x80000000);
        assertThat(ProfileCodec.parseColor("00AA11")).isNull();
        assertThat(ProfileCodec.parseColor("#GG0000")).isNull();
    }

    @Test
    void replaceAllSwapsEveryProfileInAndAppliesTheNewOne() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.system().createProfile("PvP");
        sample.amount.set(9);
        sample.setEnabled(true);
        game.system().flush();
        String saved = game.files.get("profiles/PvP.json");
        assertThat(saved).isNotNull();

        JsonObject data = new JsonObject();
        data.add("Fresh", JsonParser.parseString(saved));
        assertThat(game.system().replaceProfiles(Profiles.Transfer.of("Fresh", data)))
                .isTrue();

        assertThat(game.system().profiles()).containsExactly("Fresh");
        assertThat(game.system().activeProfile()).isEqualTo("Fresh");
        assertThat(sample.amount.get()).isEqualTo(9);
        assertThat(sample.isEnabled()).isTrue();
        assertThat(game.files).containsKey("profiles/Fresh.json");
        assertThat(game.files).doesNotContainKey("profiles/PvP.json");
    }

    @Test
    void replaceAllWithNothingLeftFallsBackToDefaults() {
        Sample sample = new Sample();
        TestGame game = new TestGame(sample);
        game.system().createProfile("PvP");
        sample.amount.set(9);
        game.system().flush();

        assertThat(game.system().replaceProfiles(Profiles.Transfer.of("PvP", new JsonObject())))
                .isTrue();
        assertThat(game.system().profiles()).containsExactly("Default");
        assertThat(game.system().activeProfile()).isEqualTo("Default");
        assertThat(sample.amount.get()).isEqualTo(3);
        assertThat(game.files).doesNotContainKey("profiles/PvP.json");
    }
}
