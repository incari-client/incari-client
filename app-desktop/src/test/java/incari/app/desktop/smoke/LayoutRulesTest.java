package incari.app.desktop.smoke;

import static org.assertj.core.api.Assertions.assertThat;

import incari.app.desktop.smoke.LayoutRules.Box;
import java.util.List;
import org.junit.jupiter.api.Test;

class LayoutRulesTest {

    private static final Box DONE = new Box("Done", 10, 10, 110, 30);
    private static final Box RESET = new Box("Reset", 114, 10, 164, 30);

    @Test
    void controlsSideBySideAreFine() {
        Box label = new Box("Done text", 45, 16, 75, 25);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE, RESET), List.of(label)))
                .isEmpty();
    }

    @Test
    void widgetsOverEachOtherFail() {
        Box over = new Box("Over", 100, 10, 150, 30);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE, over), List.of()))
                .singleElement()
                .asString()
                .contains("Done", "overlaps", "Over");
    }

    @Test
    void widgetsTouchingEdgesAreFine() {
        Box next = new Box("Next", 110, 10, 160, 30);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE, next), List.of()))
                .isEmpty();
    }

    @Test
    void aWidgetOffTheScreenFails() {
        Box wide = new Box("Wide", 200, 10, 340, 30);
        assertThat(LayoutRules.problems(320, 240, List.of(wide), List.of()))
                .singleElement()
                .asString()
                .contains("runs off the screen");
    }

    @Test
    void textPastItsButtonFails() {
        Box long_ = new Box("A very long label", 14, 16, 130, 25);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE, RESET), List.of(long_)))
                .anyMatch(problem -> problem.contains("runs past Done"));
    }

    @Test
    void itsShadowMayReachAUnitPast() {
        Box snug = new Box("Snug", 12, 16, 111, 26);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE), List.of(snug))).isEmpty();
    }

    @Test
    void aLabelRunningIntoAControlFails() {
        Box label = new Box("A row's long label", 0, 50, 140, 59);
        Box toggle = new Box("Toggle", 120, 45, 170, 65);
        assertThat(LayoutRules.problems(320, 240, List.of(toggle), List.of(label)))
                .singleElement()
                .asString()
                .contains("runs into Toggle");
    }

    @Test
    void textOverTextFails() {
        Box title = new Box("Title", 100, 20, 200, 29);
        Box status = new Box("Status", 120, 24, 220, 33);
        assertThat(LayoutRules.problems(320, 240, List.of(), List.of(title, status)))
                .singleElement()
                .asString()
                .contains("overlaps");
    }

    @Test
    void emptyBoxesAreIgnored() {
        Box hidden = new Box("Hidden", 50, 50, 50, 70);
        assertThat(LayoutRules.problems(320, 240, List.of(DONE, hidden), List.of()))
                .isEmpty();
    }

    @Test
    void aScrollingScreenMayRunPastTheBottom() {
        Box low = new Box("Low", 10, 230, 110, 250);
        assertThat(LayoutRules.problems(320, 240, List.of(low), List.of())).hasSize(1);
        assertThat(LayoutRules.problems(320, 240, true, List.of(low), List.of()))
                .isEmpty();
    }
}
