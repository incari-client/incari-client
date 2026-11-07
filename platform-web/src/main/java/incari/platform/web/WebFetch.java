package incari.platform.web;

import java.io.IOException;
import org.jspecify.annotations.Nullable;
import org.teavm.interop.Async;
import org.teavm.interop.AsyncCallback;
import org.teavm.jso.JSBody;
import org.teavm.jso.JSFunctor;
import org.teavm.jso.JSObject;
import org.teavm.jso.typedarrays.ArrayBuffer;
import org.teavm.jso.typedarrays.Int8Array;

/**
 * HTTP GETs through the browser's {@code fetch}, for {@link WebSocketNetwork#fetch}. The calling
 * thread suspends until the reply is in; other threads, and frames, carry on meanwhile.
 */
final class WebFetch {

    private WebFetch() {}

    /**
     * The body of {@code url}, at most {@code maxBytes}. Suspends the calling thread; never call it
     * on the render thread.
     */
    static byte[] get(String url, int maxBytes) throws IOException {
        JSObject reply = request(url, maxBytes);
        String error = error(reply);
        if (error != null) {
            throw new IOException(error);
        }
        return new Int8Array(data(reply)).copyToJavaArray();
    }

    @Async
    private static native JSObject request(String url, int maxBytes);

    private static void request(String url, int maxBytes, AsyncCallback<JSObject> callback) {
        start(url, maxBytes, callback::complete);
    }

    @JSFunctor
    interface Done extends JSObject {
        void done(JSObject reply);
    }

    /**
     * Starts the fetch; {@code done} hears {@code {data}} or {@code {error}}, never both. A body
     * the server says is too big is refused before it's read. A refused cross-origin fetch fails
     * with no reason the page may see, so the error names the likely one.
     */
    @JSBody(
            params = {"url", "maxBytes", "done"},
            script =
                    "var tooBig = function(n) { return new Error('the file is ' + n + ' bytes, over the limit of ' + maxBytes); };"
                            + "fetch(url, {credentials: 'omit'}).then(function(r) {"
                            + "  if (!r.ok) throw new Error('HTTP ' + r.status);"
                            + "  var length = Number(r.headers.get('Content-Length'));"
                            + "  if (length > maxBytes) { if (r.body) r.body.cancel(); throw tooBig(length); }"
                            + "  return r.arrayBuffer(); },"
                            + " function(e) { throw new Error(e + ' (offline, or the host doesn\\'t let web pages download it)'); })"
                            + ".then(function(b) { if (b.byteLength > maxBytes) throw tooBig(b.byteLength); done({data: b}); })"
                            + ".catch(function(e) { done({error: 'Couldn\\'t fetch ' + url + ': ' + (e && e.message || e)}); });")
    private static native void start(String url, int maxBytes, Done done);

    @JSBody(params = "reply", script = "return reply.error ? String(reply.error) : null;")
    private static native @Nullable String error(JSObject reply);

    @JSBody(params = "reply", script = "return reply.data;")
    private static native ArrayBuffer data(JSObject reply);
}
