package incari.client.modules;

import com.mojang.logging.LogUtils;
import incari.client.platform.ClientPlatform;
import incari.client.ui.ControlList;
import incari.client.ui.ListScreen;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.platform.api.PlatformShell;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.function.BiConsumer;
import java.util.stream.Stream;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * A module's file chooser ({@code Game.chooseFile}): the files in one folder of the game directory,
 * newest first, each with a button to choose it. The folder can be opened beside the game; in the
 * browser that opens the file picker instead, and the files picked (or dropped on the window) are
 * copied into the folder.
 */
final class FileChooserScreen extends ListScreen {

    private static final Logger LOGGER = LogUtils.getLogger();

    /** A file bigger than this isn't offered: it would only be refused after a long read. */
    private static final long MAX_SIZE = 64L << 20;

    /** Three side by side fit the narrowest screen, 320 GUI units. */
    private static final int FOOTER_BUTTON = 90;

    private final Path folder;
    private final List<String> extensions;
    private final BiConsumer<String, byte[]> chosen;
    private final List<Path> files = new ArrayList<>();
    private Component caption;
    private long revision;

    FileChooserScreen(
            @Nullable Screen parent,
            String title,
            Path folder,
            List<String> extensions,
            BiConsumer<String, byte[]> chosen) {
        super(Component.literal(title), parent);
        this.folder = folder;
        this.extensions = List.copyOf(extensions);
        this.chosen = chosen;
        this.caption = defaultCaption();
        list();
    }

    private Component defaultCaption() {
        String kinds = String.join(", ", extensions);
        return Component.literal(
                ClientPlatform.get().shell().showsFolders()
                        ? "Put " + kinds + " files in the folder to see them here."
                        : "Add " + kinds + " files from your computer, or drop them here.");
    }

    /** Reads the folder's files again. */
    private void list() {
        files.clear();
        if (Files.isDirectory(folder)) {
            try (Stream<Path> listed = Files.list(folder)) {
                listed.filter(path -> Files.isRegularFile(path)
                                && accepted(path.getFileName().toString()))
                        .sorted(Comparator.comparingLong(FileChooserScreen::modified)
                                .reversed())
                        .forEach(files::add);
            } catch (IOException e) {
                LOGGER.warn("Couldn't list {}: {}", folder, e.toString());
            }
        }
        revision++;
    }

    private boolean accepted(String name) {
        String lower = name.toLowerCase(Locale.ROOT);
        for (String extension : extensions) {
            if (lower.endsWith(extension)) {
                return true;
            }
        }
        return false;
    }

    private static long modified(Path path) {
        try {
            return Files.getLastModifiedTime(path).toMillis();
        } catch (IOException e) {
            return 0;
        }
    }

    private static long size(Path path) {
        try {
            return Files.size(path);
        } catch (IOException e) {
            return -1;
        }
    }

    @Override
    protected Component caption() {
        return caption;
    }

    @Override
    protected long revision() {
        return revision;
    }

    @Override
    protected Object shape() {
        return List.copyOf(files);
    }

    @Override
    protected List<ControlList.Row> rows(ControlList list) {
        List<ControlList.Row> rows = new ArrayList<>();
        if (files.isEmpty()) {
            rows.add(list.heading(Component.literal("No files yet").withColor(Theme.MUTED)));
        }
        int labelWidth = list.getRowWidth() - Theme.SMALL_BUTTON_WIDTH - Theme.GAP * 4;
        for (Path file : files) {
            String name = file.getFileName().toString();
            long bytes = size(file);
            Button choose = Ui.button(Component.literal("Load"), Theme.SMALL_BUTTON_WIDTH, () -> choose(file));
            choose.active = bytes >= 0 && bytes <= MAX_SIZE;
            String label = Ui.fit(this.font, name, labelWidth);
            rows.add(list.row(Component.literal(label), name + ", " + sizeText(bytes), List.of(choose), () -> {}));
        }
        return rows;
    }

    /** "2.4 MB", "830 KB". */
    static String sizeText(long bytes) {
        if (bytes < 0) {
            return "unreadable";
        }
        if (bytes >= 1L << 20) {
            return String.format(Locale.ROOT, "%.1f MB", bytes / (double) (1L << 20));
        }
        return Math.max(1, (bytes + 1023) / 1024) + " KB";
    }

    private void choose(Path file) {
        byte[] bytes;
        try {
            bytes = Files.readAllBytes(file);
        } catch (IOException e) {
            LOGGER.warn("Couldn't read {}: {}", file, e.toString());
            caption = Component.literal("Couldn't read " + file.getFileName()).withStyle(ChatFormatting.RED);
            return;
        }
        this.minecraft.gui.setScreen(parent);
        chosen.accept(file.getFileName().toString(), bytes);
    }

    @Override
    protected void footer(LinearLayout footer) {
        PlatformShell shell = ClientPlatform.get().shell();
        footer.addChild(Ui.button(
                Component.literal(shell.showsFolders() ? "Open folder" : "Add files"), FOOTER_BUTTON, () -> {
                    try {
                        Files.createDirectories(folder);
                    } catch (IOException e) {
                        LOGGER.warn("Couldn't make {}: {}", folder, e.toString());
                    }
                    shell.openPath(folder.toAbsolutePath().toString());
                }));
        footer.addChild(Ui.button(Component.literal("Refresh"), FOOTER_BUTTON, this::list));
        footer.addChild(Ui.button(CommonComponents.GUI_CANCEL, FOOTER_BUTTON, this::onClose));
    }

    /** Files dropped on the window, or picked in the browser's file picker: copied into the folder. */
    @Override
    public void onFilesDrop(List<Path> dropped) {
        int added = 0;
        int skipped = 0;
        for (Path file : dropped) {
            Path name = file.getFileName();
            if (name == null || !accepted(name.toString()) || !Files.isRegularFile(file)) {
                skipped++;
                continue;
            }
            try {
                Files.createDirectories(folder);
                Path target = folder.resolve(name.toString());
                if (!target.toAbsolutePath()
                        .normalize()
                        .equals(file.toAbsolutePath().normalize())) {
                    // Read and written whole: the browser's in-memory files have no copy of their own.
                    Files.write(target, Files.readAllBytes(file));
                }
                added++;
            } catch (IOException e) {
                LOGGER.warn("Couldn't copy {} into {}: {}", file, folder, e.toString());
                skipped++;
            }
        }
        if (skipped > 0) {
            caption = Component.literal(skipped + (skipped == 1 ? " file wasn't" : " files weren't") + " added: only "
                            + String.join(", ", extensions))
                    .withStyle(ChatFormatting.GOLD);
        } else if (added > 0) {
            caption = defaultCaption();
        }
        list();
    }
}
