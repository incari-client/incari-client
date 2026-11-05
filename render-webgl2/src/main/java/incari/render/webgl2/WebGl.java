package incari.render.webgl2;

import org.jspecify.annotations.Nullable;
import org.teavm.jso.JSBody;
import org.teavm.jso.JSObject;
import org.teavm.jso.typedarrays.ArrayBufferView;
import org.teavm.jso.typedarrays.Int8Array;
import org.teavm.jso.webgl.WebGL2RenderingContext;
import org.teavm.jso.webgl.WebGLProgram;

/**
 * Every JavaScript snippet the WebGL2 backend runs, in one file so the page surface is easy to
 * audit (like {@code platform-web}'s {@code Dom}). Everything else uses TeaVM's typed
 * {@link WebGL2RenderingContext}.
 */
final class WebGl {

    /** EXT_texture_filter_anisotropic's enums. */
    static final int TEXTURE_MAX_ANISOTROPY = 0x84FE;

    static final int MAX_TEXTURE_MAX_ANISOTROPY = 0x84FF;

    /** WEBGL_debug_renderer_info's enums. */
    static final int UNMASKED_VENDOR = 0x9245;

    static final int UNMASKED_RENDERER = 0x9246;

    private WebGl() {}

    @JSBody(script = "return typeof WebGL2RenderingContext !== 'undefined';")
    static native boolean isSupported();

    /**
     * The canvas's WebGL2 context, or null if the browser won't make one. The game draws into its
     * own framebuffers and blits to the canvas once a frame, so the canvas gets no depth, stencil,
     * alpha or antialiasing (each costs memory and a resolve on low-end GPUs).
     */
    @JSBody(
            params = "canvasId",
            script = "var canvas = document.getElementById(canvasId);"
                    + "if (!canvas || !canvas.getContext) return null;"
                    + "var gl = canvas.getContext('webgl2', { alpha: false, depth: false, stencil: false,"
                    + " antialias: false, premultipliedAlpha: false, preserveDrawingBuffer: false,"
                    + " powerPreference: 'high-performance' }) || null;"
                    + "if (gl && !canvas.__incariGlWatched) { canvas.__incariGlWatched = true;"
                    // Cancelled, so the browser may give the context back. What was on the GPU
                    // (textures, meshes, shaders) isn't: the game keeps no copies to upload again,
                    // so the page must be reloaded. The platform's event queue hears of it through
                    // the canvas's sink (platform-web's Dom.listen) and reports the window
                    // minimized: the game stops drawing and, minimized, saves its singleplayer world.
                    + "  canvas.addEventListener('webglcontextlost', function(e) { e.preventDefault();"
                    + "    console.error('[Incari] The WebGL context was lost; the page must be reloaded');"
                    + "    var sink = canvas.__incariSink; if (sink) sink('graphics', '', '', 1, 0, 0, 0, 0, 0);"
                    + "    if (document.pointerLockElement && document.exitPointerLock) document.exitPointerLock();"
                    + "    if (document.fullscreenElement && document.exitFullscreen) {"
                    + "      var p = document.exitFullscreen(); if (p && p.catch) p.catch(function() {}); }"
                    + "    if (document.getElementById('incari-graphics-lost')) return;"
                    + "    var box = document.createElement('div'); box.id = 'incari-graphics-lost';"
                    + "    box.setAttribute('role', 'alert');"
                    + "    box.style.cssText = 'position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);"
                    + "max-width:min(420px,90vw);padding:16px 20px;border-radius:8px;background:rgba(0,0,0,0.85);"
                    + "color:#eee;font:15px/1.4 system-ui,sans-serif;text-align:center;z-index:2147483647';"
                    + "    var text = document.createElement('p'); text.style.margin = '0 0 12px';"
                    + "    text.textContent = 'The graphics stopped working (the browser reset them). "
                    + "Singleplayer worlds are being saved. Reload the page to keep playing.';"
                    + "    var button = document.createElement('button'); button.textContent = 'Reload';"
                    + "    button.style.cssText = 'font:inherit;padding:6px 18px;cursor:pointer';"
                    + "    button.addEventListener('click', function() { location.reload(); });"
                    + "    box.appendChild(text); box.appendChild(button); document.body.appendChild(box); });"
                    + "  canvas.addEventListener('webglcontextrestored', function() {"
                    + "    console.warn('[Incari] The WebGL context came back, empty; reload the page to draw again'); }); }"
                    + "return gl;")
    static native @Nullable WebGL2RenderingContext context(String canvasId);

    /** Turns an extension on; false if the browser doesn't have it. */
    @JSBody(
            params = {"gl", "name"},
            script = "return !!gl.getExtension(name);")
    static native boolean enableExtension(WebGL2RenderingContext gl, String name);

    /**
     * Makes clip space's depth run from 0 to 1, as the game's desktop backends set it up
     * ({@code glClipControl}), with EXT_clip_control; false if the browser doesn't have it.
     */
    @JSBody(
            params = {"gl"},
            script = "var e = gl.getExtension('EXT_clip_control'); if (!e) return false;"
                    + " e.clipControlEXT(e.LOWER_LEFT_EXT, e.ZERO_TO_ONE_EXT); return true;")
    static native boolean clipControlZeroToOne(WebGL2RenderingContext gl);

    /** WEBGL_polygon_mode, for the game's wireframe debug view, or null where the browser lacks it. */
    @JSBody(
            params = {"gl"},
            script = "return gl.getExtension('WEBGL_polygon_mode');")
    static native @Nullable JSObject polygonModeExtension(WebGL2RenderingContext gl);

    /** Draws triangles as their outlines ({@code line}) or filled, through WEBGL_polygon_mode. */
    @JSBody(
            params = {"extension", "line"},
            script = "extension.polygonModeWEBGL(0x0408, line ? 0x1B01 : 0x1B02);")
    static native void polygonMode(JSObject extension, boolean line);

    @JSBody(
            params = {"gl"},
            script = "return gl.getSupportedExtensions() || [];")
    static native JSObject supportedExtensions(WebGL2RenderingContext gl);

    @JSBody(
            params = {"list"},
            script = "return list.length;")
    static native int length(JSObject list);

    @JSBody(
            params = {"list", "index"},
            script = "return String(list[index]);")
    static native String stringAt(JSObject list, int index);

    /** A linked program's uniform block's size in bytes ({@code UNIFORM_BLOCK_DATA_SIZE}). */
    @JSBody(
            params = {"gl", "program", "block"},
            script = "return gl.getActiveUniformBlockParameter(program, block, 0x8A40) | 0;")
    static native int uniformBlockDataSize(WebGL2RenderingContext gl, WebGLProgram program, int block);

    /** A string parameter, or "" where the browser hides it (the unmasked renderer, for one). */
    @JSBody(
            params = {"gl", "name"},
            script = "var v = gl.getParameter(name); return v == null ? '' : String(v);")
    static native String stringParameter(WebGL2RenderingContext gl, int name);

    /** A numeric parameter, or 0 where the browser has none. */
    @JSBody(
            params = {"gl", "name"},
            script = "var v = gl.getParameter(name); return typeof v === 'number' ? v : 0;")
    static native double numberParameter(WebGL2RenderingContext gl, int name);

    @JSBody(
            params = {"gl"},
            script = "return gl.isContextLost();")
    static native boolean isContextLost(WebGL2RenderingContext gl);

    /**
     * The same bytes as the typed array a pixel {@code type} needs: WebGL refuses texture uploads
     * and read-backs through any other kind (an {@code Int8Array} for {@code UNSIGNED_BYTE} pixels
     * is an error). {@code view} must start on the element size, which staging views do.
     */
    @JSBody(
            params = {"view", "type"},
            script = "var C = type === 0x1401 ? Uint8Array : type === 0x1400 ? Int8Array"
                    + " : (type === 0x1403 || type === 0x140B) ? Uint16Array : type === 0x1402 ? Int16Array"
                    + " : type === 0x1404 ? Int32Array : type === 0x1406 ? Float32Array : Uint32Array;"
                    + "return new C(view.buffer, view.byteOffset, view.byteLength / C.BYTES_PER_ELEMENT);")
    static native ArrayBufferView typed(ArrayBufferView view, int type);

    /** {@link #typed}, or null if {@code view} doesn't start on the element size. */
    @JSBody(
            params = {"view", "type"},
            script = "var C = type === 0x1401 ? Uint8Array : type === 0x1400 ? Int8Array"
                    + " : (type === 0x1403 || type === 0x140B) ? Uint16Array : type === 0x1402 ? Int16Array"
                    + " : type === 0x1404 ? Int32Array : type === 0x1406 ? Float32Array : Uint32Array;"
                    + "if (view.byteOffset % C.BYTES_PER_ELEMENT !== 0) return null;"
                    + "return new C(view.buffer, view.byteOffset, view.byteLength / C.BYTES_PER_ELEMENT);")
    static native @Nullable ArrayBufferView typedIfAligned(ArrayBufferView view, int type);

    /**
     * Publishes the device's counters as {@code window.__incariWebGl}, for tools and the console to
     * read ({@code BrowserCheck}'s {@code eval:} step): frames, draws and skipped draws so far,
     * and the last blit's source and canvas sizes. Once a frame, so it costs one call.
     */
    @JSBody(
            params = {"frames", "draws", "skipped", "width", "height", "canvasWidth", "canvasHeight"},
            script = "window.__incariWebGl = { frames: frames, draws: draws, skipped: skipped,"
                    + " blit: width + 'x' + height + ' -> ' + canvasWidth + 'x' + canvasHeight };")
    static native void publishStats(
            int frames, int draws, int skipped, int width, int height, int canvasWidth, int canvasHeight);

    /** An extension's object, or null where the browser doesn't have it. */
    @JSBody(
            params = {"gl", "name"},
            script = "return gl.getExtension(name) || null;")
    static native @Nullable JSObject extension(WebGL2RenderingContext gl, String name);

    // --- GPU timing (EXT_disjoint_timer_query_webgl2) ---------------------------------

    @JSBody(params = "gl", script = "return gl.createQuery();")
    static native JSObject createQuery(WebGL2RenderingContext gl);

    /** Starts timing the GPU's work into {@code query} (TIME_ELAPSED_EXT). */
    @JSBody(
            params = {"gl", "query"},
            script = "gl.beginQuery(0x88BF, query);")
    static native void beginTimer(WebGL2RenderingContext gl, JSObject query);

    @JSBody(params = "gl", script = "gl.endQuery(0x88BF);")
    static native void endTimer(WebGL2RenderingContext gl);

    /**
     * A finished timer's result in nanoseconds; -1 if it isn't ready yet, -2 if the GPU was
     * disjoint meanwhile (the result is meaningless: a power change, a context switch).
     */
    @JSBody(
            params = {"gl", "query"},
            script = "if (!gl.getQueryParameter(query, 0x8867)) return -1;"
                    + "if (gl.getParameter(0x8FBB)) return -2;"
                    + "return gl.getQueryParameter(query, 0x8866);")
    static native double timerResult(WebGL2RenderingContext gl, JSObject query);

    // --- Multi-draw (WEBGL_multi_draw) --------------------------------------------------

    /**
     * Views of two buffers of little-endian ints in linear memory, kept for
     * {@link #multiDrawElements}: made once, not per call (a hot path), and made again only when
     * growing the module's memory detaches them.
     */
    @JSBody(
            params = {"counts", "offsets"},
            script = "return { counts: new Int32Array(counts.buffer, counts.byteOffset, counts.byteLength >> 2),"
                    + " offsets: new Int32Array(offsets.buffer, offsets.byteOffset, offsets.byteLength >> 2) };")
    static native JSObject intViews(Int8Array counts, Int8Array offsets);

    /**
     * Draws {@code count} index ranges in one call: each sub-draw's index count and byte offset
     * from {@code views} ({@link #intViews}). The shader tells sub-draws apart by
     * {@code gl_DrawID}. False, drawing nothing, if the views were detached (make them again).
     */
    @JSBody(
            params = {"extension", "mode", "views", "type", "count"},
            script =
                    "if (views.counts.length === 0) return false;"
                            + "extension.multiDrawElementsWEBGL(mode, views.counts, 0, type, views.offsets, 0, count); return true;")
    static native boolean multiDrawElements(JSObject extension, int mode, JSObject views, int type, int count);

    /**
     * Uploads the first {@code bytes} of {@code view} (a kept view of a buffer in linear memory)
     * to {@code target} at {@code offset}. False, uploading nothing, if the view was detached.
     */
    @JSBody(
            params = {"gl", "target", "offset", "view", "bytes"},
            script = "if (view.length === 0 && bytes > 0) return false;"
                    + "gl.bufferSubData(target, offset, view, 0, bytes); return true;")
    static native boolean bufferSubData(WebGL2RenderingContext gl, int target, int offset, Int8Array view, int bytes);

    // --- Uploads straight from linear memory (ADR 0027) --------------------------------------

    /**
     * Views of the whole linear memory {@code view} is in, one per element type WebGL uploads read,
     * remade only when memory growth detaches them. Every JavaScript object handed to Java is kept
     * in the runtime's weak map, so a view per upload put thousands of entries there for each major
     * collection to walk in its pause.
     */
    @JSBody(
            params = "view",
            script = "var b = view.buffer; return { buffer: b, i8: new Int8Array(b), u8: new Uint8Array(b),"
                    + " i16: new Int16Array(b), u16: new Uint16Array(b), i32: new Int32Array(b),"
                    + " u32: new Uint32Array(b), f32: new Float32Array(b) };")
    static native JSObject memoryViews(Int8Array view);

    /**
     * Uploads {@code bytes} from {@code address} of linear memory to {@code target} at
     * {@code offset}, through {@code views} ({@link #memoryViews}). False, uploading nothing, if
     * growing the memory detached them.
     */
    @JSBody(
            params = {"gl", "target", "offset", "views", "address", "bytes"},
            script = "if (views.buffer.byteLength === 0) return false;"
                    + "gl.bufferSubData(target, offset, views.i8, address, bytes); return true;")
    static native boolean bufferSubDataAt(
            WebGL2RenderingContext gl, int target, int offset, JSObject views, int address, int bytes);

    /**
     * {@code texSubImage2D} with the pixels from {@code address} of linear memory, read through the
     * view {@code type} needs (as {@link #typed} picks it). 1 when uploaded; 0, uploading nothing,
     * if growing the memory detached the views; 2 if {@code address} isn't on the element size,
     * which a typed array's offset must be.
     */
    @JSBody(
            params = {"gl", "target", "level", "x", "y", "width", "height", "format", "type", "views", "address"},
            script = "if (views.buffer.byteLength === 0) return 0;"
                    + "var v = type === 0x1401 ? views.u8 : type === 0x1400 ? views.i8"
                    + " : (type === 0x1403 || type === 0x140B) ? views.u16 : type === 0x1402 ? views.i16"
                    + " : type === 0x1404 ? views.i32 : type === 0x1406 ? views.f32 : views.u32;"
                    + "if (address % v.BYTES_PER_ELEMENT !== 0) return 2;"
                    + "gl.texSubImage2D(target, level, x, y, width, height, format, type, v,"
                    + " address / v.BYTES_PER_ELEMENT); return 1;")
    static native int texSubImage2DAt(
            WebGL2RenderingContext gl,
            int target,
            int level,
            int x,
            int y,
            int width,
            int height,
            int format,
            int type,
            JSObject views,
            int address);

    /**
     * Uploads {@code bytes} of {@code view} (a kept view of a buffer in linear memory) from byte
     * {@code from} to {@code target} at {@code offset}. False, uploading nothing, if the view was detached.
     */
    @JSBody(
            params = {"gl", "target", "offset", "view", "from", "bytes"},
            script = "if (view.length === 0 && bytes > 0) return false;"
                    + "gl.bufferSubData(target, offset, view, from, bytes); return true;")
    static native boolean bufferSubDataFrom(
            WebGL2RenderingContext gl, int target, int offset, Int8Array view, int from, int bytes);

    /**
     * Uploads {@code indices} 32-bit indices of sequential quads (0 1 2 2 3 0, then 4 5 6 6 7 4,
     * and on) into the bound element array buffer, sized to hold exactly those.
     */
    @JSBody(
            params = {"gl", "indices"},
            script = "var data = new Uint32Array(indices);"
                    + "for (var i = 0, v = 0; i + 6 <= indices; i += 6, v += 4) {"
                    + "  data[i] = v; data[i + 1] = v + 1; data[i + 2] = v + 2;"
                    + "  data[i + 3] = v + 2; data[i + 4] = v + 3; data[i + 5] = v; }"
                    + "gl.bufferData(0x8893, data, 0x88E4);")
    static native void uploadQuadIndices(WebGL2RenderingContext gl, int indices);

    @JSBody(params = "message", script = "console.warn(message);")
    static native void warn(String message);

    @JSBody(params = "message", script = "console.info(message);")
    static native void info(String message);

    @JSBody(params = "message", script = "console.error(message);")
    static native void error(String message);
}
