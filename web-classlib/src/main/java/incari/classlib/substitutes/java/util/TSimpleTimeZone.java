package incari.classlib.substitutes.java.util;

import java.util.Date;
import java.util.TimeZone;

/**
 * {@code java.util.SimpleTimeZone}, which TeaVM lacks, as a fixed offset without daylight saving:
 * all Bouncy Castle asks of it, formatting ASN.1 times in GMT (ADR 0070). The rule-based
 * constructors aren't here.
 */
public class TSimpleTimeZone extends TimeZone {

    private static final long serialVersionUID = 1L;

    private int rawOffset;

    @SuppressWarnings("this-escape") // setID is TimeZone's, and reads nothing of this class.
    public TSimpleTimeZone(int rawOffset, String id) {
        this.rawOffset = rawOffset;
        setID(id);
    }

    @Override
    public int getOffset(int era, int year, int month, int day, int dayOfWeek, int milliseconds) {
        return rawOffset;
    }

    @Override
    public int getOffset(long date) {
        return rawOffset;
    }

    @Override
    public int getRawOffset() {
        return rawOffset;
    }

    @Override
    public void setRawOffset(int offsetMillis) {
        rawOffset = offsetMillis;
    }

    @Override
    public boolean useDaylightTime() {
        return false;
    }

    @Override
    public boolean inDaylightTime(Date date) {
        return false;
    }

    @Override
    public boolean hasSameRules(TimeZone other) {
        return other != null && other.getRawOffset() == rawOffset && !other.useDaylightTime();
    }
}
