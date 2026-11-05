/**
 * Microsoft account sign-in for online-mode servers (ADR 0070): the device-code flow with the
 * Minecraft launcher's client id, Xbox Live and XSTS, the Minecraft token and profile, session
 * joins and chat-signing keys, all over {@link incari.net.https.HttpsClient}. No game types, no
 * blocking, and never a token in a log or an exception message.
 */
@NullMarked
package incari.net.msa;

import org.jspecify.annotations.NullMarked;
