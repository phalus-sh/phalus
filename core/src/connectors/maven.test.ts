import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanMaven } from './maven.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-maven-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanMaven', () => {
  it('returns empty array when no pom.xml', () => {
    expect(scanMaven(tmpDir)).toEqual([]);
  });

  it('parses basic dependencies', () => {
    fs.writeFileSync(path.join(tmpDir, 'pom.xml'), `
<project>
  <dependencies>
    <dependency>
      <groupId>com.google.guava</groupId>
      <artifactId>guava</artifactId>
      <version>31.1-jre</version>
    </dependency>
    <dependency>
      <groupId>org.springframework</groupId>
      <artifactId>spring-core</artifactId>
      <version>6.0.0</version>
    </dependency>
  </dependencies>
</project>
`);
    const pkgs = scanMaven(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('com.google.guava:guava');
    expect(pkgs.map(p => p.name)).toContain('org.springframework:spring-core');
    const guava = pkgs.find(p => p.name === 'com.google.guava:guava')!;
    expect(guava.version).toBe('31.1-jre');
    expect(guava.ecosystem).toBe('maven');
  });

  it('skips test-scoped dependencies', () => {
    fs.writeFileSync(path.join(tmpDir, 'pom.xml'), `
<project>
  <dependencies>
    <dependency>
      <groupId>junit</groupId>
      <artifactId>junit</artifactId>
      <version>4.13.2</version>
      <scope>test</scope>
    </dependency>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>mylib</artifactId>
      <version>1.0.0</version>
    </dependency>
  </dependencies>
</project>
`);
    const pkgs = scanMaven(tmpDir);
    expect(pkgs.map(p => p.name)).not.toContain('junit:junit');
    expect(pkgs.map(p => p.name)).toContain('com.example:mylib');
  });

  it('parses dependencyManagement section', () => {
    fs.writeFileSync(path.join(tmpDir, 'pom.xml'), `
<project>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>org.apache.commons</groupId>
        <artifactId>commons-lang3</artifactId>
        <version>3.12.0</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
</project>
`);
    const pkgs = scanMaven(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('org.apache.commons:commons-lang3');
    expect(pkgs[0]!.version).toBe('3.12.0');
  });

  it('extracts project license and applies it', () => {
    fs.writeFileSync(path.join(tmpDir, 'pom.xml'), `
<project>
  <licenses>
    <license>
      <name>Apache-2.0</name>
    </license>
  </licenses>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>lib</artifactId>
      <version>1.0.0</version>
    </dependency>
  </dependencies>
</project>
`);
    const pkgs = scanMaven(tmpDir);
    const lib = pkgs.find(p => p.name === 'com.example:lib')!;
    expect(lib.licenseExpression).toBe('Apache-2.0');
    expect(lib.licenseSource).toBe('pom.xml');
  });

  it('deduplicates dependencies', () => {
    fs.writeFileSync(path.join(tmpDir, 'pom.xml'), `
<project>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>lib</artifactId>
      <version>1.0.0</version>
    </dependency>
  </dependencies>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>com.example</groupId>
        <artifactId>lib</artifactId>
        <version>1.0.0</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
</project>
`);
    const pkgs = scanMaven(tmpDir);
    const libs = pkgs.filter(p => p.name === 'com.example:lib');
    expect(libs).toHaveLength(1);
  });
});
