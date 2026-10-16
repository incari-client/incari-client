package incari.features.zoom;

import static com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes;

import com.tngtech.archunit.core.importer.ImportOption;
import com.tngtech.archunit.junit.AnalyzeClasses;
import com.tngtech.archunit.junit.ArchTest;
import com.tngtech.archunit.lang.ArchRule;

/** A feature sees the module framework and the JDK's basics: no game, platform or other feature. */
@AnalyzeClasses(packages = "incari.features.zoom", importOptions = ImportOption.DoNotIncludeTests.class)
class BoundaryTest {

    @ArchTest
    static final ArchRule onlyTheFramework = classes()
            .should()
            .onlyDependOnClassesThat()
            .resideInAnyPackage(
                    "java.lang..", "java.util..", "org.jspecify..", "incari.framework..", "incari.features.zoom..");
}
