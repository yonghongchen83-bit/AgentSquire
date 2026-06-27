import type { Options } from '@wdio/types'
import path from 'path'

export const config: Options.Testrunner = {
  hostname: '127.0.0.1',
  port: 4444,

  specs: ['./e2e/specs/**/*.ts'],
  exclude: [],

  maxInstances: 1,

  capabilities: [{
    browserName: 'tauri',
  }],

  logLevel: 'info',
  outputDir: './e2e-results',

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },

  reporters: ['spec'],

  autoCompileOpts: {
    tsNodeOpts: {
      project: './e2e/tsconfig.json',
    },
  },

  beforeSession: () => {
    require('expect-webdriverio').setOptions({ wait: 5000 })
  },

  afterTest: async (test, context, { error }) => {
    if (error) {
      const timestamp = Date.now()
      const screenshotPath = `./e2e-results/screenshots/error-${timestamp}.png`
      await browser.saveScreenshot(screenshotPath)
    }
  },
}
