<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useAct, attempt } from '../act';
import { useApi } from '../core/api';
import { ACCOUNT_KIND, accountLabel, accountState } from '../accounts';
import { catalog } from '../catalog';
import Icon from './Icon.vue';
import InboxCallout from './InboxCallout.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import NavRow from './NavRow.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import StandingBadge from './StandingBadge.vue';
import Tip from './Tip.vue';
import Until from './Until.vue';
import { Button } from '@/components/ui/button';

// The Claude accounts agents run on (catalog.health.accounts, as the server reports them, each with its kind): the machine's
// own login, the logins the owner added, each known by the address it signed in as, and the accounts a plugin provides, one
// for each of its connections, each known by that plugin and connection. Each says whether it takes work now and, when not,
// why and how long until the limit it hit resets; one that takes work but whose limit the provider warns is near says which
// limit, how much of it is used and when it resets, as the server judged it (nearLimit). A card runs on one account and moves to another when its own is out, so the
// page says which can take work. Adding and removing a login are the owner's rails, buttons here: Add account has the server
// run the sign-in and puts its link and the field for its code in an inbox item, shown here while it waits (the code goes to
// the sign-in and to no agent); the account is kept under its address when the sign-in ends, and adding an address that
// exists signs it in again. Remove is offered for a login only; a plugin's account goes with its connection, on the Plugins
// page its row here links to. Checking again is a button too.
const route = useRoute();
const api = useApi();
const act = useAct();
const accounts = computed(() => catalog.health?.accounts ?? []);
const account = computed(() => accounts.value.find((a) => a.id === route.params.accountId) ?? accounts.value[0]);
// The sign-in that waits for the owner: an open login item.
const signingIn = computed(() => catalog.inbox.filter((i) => i.kind === 'login' && i.status === 'open'));
// The page of the plugin the open account comes from, when the server lists that plugin.
const pluginPage = computed(() => (catalog.extensions.some((e) => e.id === account.value?.plugin) ? { name: 'extensions', params: { extensionId: account.value?.plugin } } : undefined));
</script>

<template>
  <ListPage name="accounts" :open="!!account" :title="account && accountLabel(account)" empty="Reading the accounts…">
    <template #tabs><PageTabs group="integrations" /></template>
    <template #head>
      <Button class="w-full" variant="outline" size="sm" data-new :disabled="signingIn.length > 0" @click="act('accountAdd', [], 'Open the sign-in link below')"><Icon name="plus" :size="12" /> Add account</Button>
    </template>
    <template #list>
      <NavRow v-for="a in accounts" :key="a.id" :on="account?.id === a.id" :to="{ name: 'accounts', params: { accountId: a.id } }" :description="accountState(a)" :data-account-id="a.id" :data-out="a.ok ? undefined : ''">
        <template #title><span class="min-w-0 [overflow-wrap:anywhere]">{{ accountLabel(a) }}</span><StandingBadge v-if="!a.ok" variant="warning" icon="alert" class="flex-none" data-out-badge>Out</StandingBadge><StandingBadge v-else-if="a.nearLimit" variant="warning" icon="clock" class="flex-none" data-near-badge>Near limit</StandingBadge></template>
      </NavRow>
    </template>
    <template v-if="account" #standing>
      <StandingBadge v-if="account.ok && account.nearLimit" variant="warning" icon="clock" class="flex-none" data-near-badge>Near limit</StandingBadge>
      <StandingBadge v-else-if="account.ok" variant="success" icon="check" class="flex-none" data-ok-badge>Takes work</StandingBadge>
      <StandingBadge v-else variant="warning" icon="alert" class="flex-none" data-out-badge>Out</StandingBadge>
    </template>
    <template v-if="account" #actions>
      <Tip text="Run one turn of an agent on every account now, to see which take work"><Button variant="outline" size="sm" data-check @click="attempt(() => api.healthCheck(), 'Checked again')">Check again</Button></Tip>
      <Tip v-if="account.kind === 'login'" text="Signs the account out and takes it from the pool; agents on it move to another"><Button variant="outline" size="sm" :aria-label="`Remove account ${accountLabel(account)}`" data-remove @click="act('accountRemove', [account.id], 'Account removed')">Remove</Button></Tip>
    </template>
    <PageBody>
      <div class="flex flex-col gap-4">
        <InboxCallout v-for="item in signingIn" :key="item.id" :item="item" data-sign-in />
        <div class="@container">
          <PageGrid v-if="account">
            <PageSection title="State" id="account-state" data-state>
              <KeyValues>
                <dt>Takes work</dt><dd data-takes-work>{{ account.ok ? 'Yes' : 'No' }}</dd>
                <template v-if="!account.ok"><dt>Why not</dt><dd class="[overflow-wrap:anywhere]" data-why>{{ account.error }}</dd></template>
                <template v-if="account.ok && account.nearLimit"><dt>Near its limit</dt><dd class="[overflow-wrap:anywhere]" data-near>{{ account.nearLimit }}<template v-if="account.utilization !== undefined"> · {{ Math.round(account.utilization * 100) }}% used</template></dd></template>
                <template v-if="(!account.ok || account.nearLimit) && account.resetsAt"><dt>Resets</dt><dd data-resets><Until :at="account.resetsAt" /></dd></template>
                <dt>Kind</dt><dd data-kind>{{ ACCOUNT_KIND[account.kind] }}</dd>
                <template v-if="account.plugin"><dt>Plugin</dt><dd><RouterLink v-if="pluginPage" class="text-blue-text hover:underline" :to="pluginPage" data-plugin>{{ account.plugin }}</RouterLink><span v-else data-plugin>{{ account.plugin }}</span></dd></template>
                <template v-if="account.connection"><dt>Connection</dt><dd data-connection>{{ account.connection }}</dd></template>
              </KeyValues>
            </PageSection>
          </PageGrid>
        </div>
      </div>
    </PageBody>
  </ListPage>
</template>
