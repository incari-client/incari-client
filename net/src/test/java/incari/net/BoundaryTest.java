package incari.net;

import static com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes;

import com.tngtech.archunit.core.importer.ImportOption;
import com.tngtech.archunit.junit.AnalyzeClasses;
import com.tngtech.archunit.junit.ArchTest;
import com.tngtech.archunit.lang.ArchRule;

/**
 * net sits between the game's pipeline and the platform: Netty and platform-api only. It never
 * sees game types (the client's adapters hand it what it needs) or a platform implementation, and
 * it's compiled into the browser build, so no JDK APIs beyond what TeaVM has.
 */
@AnalyzeClasses(packages = "incari.net", importOptions = ImportOption.DoNotIncludeTests.class)
class BoundaryTest {

    @ArchTest
    static final ArchRule onlyNettyAndPlatformApi = classes()
            .should()
            .onlyDependOnClassesThat()
            .resideInAnyPackage(
                    "java..",
                    "org.jspecify..",
                    "io.netty..",
                    "incari.platform.api..",
                    "incari.net..",
                    // Sign-in's TLS and JSON (ADR 0070).
                    "org.bouncycastle..",
                    "com.google.gson..")
            .allowEmptyShould(true);
}
