<script setup>
    import { ref, onMounted, reactive, toRaw } from 'vue';
    import { useRouter } from 'vue-router';
    import { ElMessage, ElMessageBox } from 'element-plus';
    import { invoke } from '@tauri-apps/api/core';
    import { useOptionsStore } from "../stores/options";
    import AccountAuthForm from '../components/AccountAuthForm.vue';
    import {handleLocalAccount, formatObjectString} from '../utils/function'

    const checks = reactive({ 
        camera: false, 
        admin: false,
        loading: true 
    });

    const activeStep = ref(0);
    const isDeploying = ref(false);
    const router = useRouter();
    const initialized = ref(false);
    const deployProgress = ref(0);
    const deployStatus = ref('');
    const isFinalizing = ref(false);
    const optionsStore = useOptionsStore();

    let is_initialized = optionsStore.getOptionByKey('is_initialized');
    if(is_initialized.index != -1 && is_initialized.data.val == 'true'){
        initialized.value = true;
    }

    let authForm = reactive({
        username: '',
        password: '',
        accountType: 'local'
    });

    // 获取当前用户名
    invoke('get_now_username').then((data)=>{
        if(data.code == 200){
            authForm.username = data.data.username;
        }
    })

    // 步骤切换
    const handleNextStep = () => {
        if (activeStep.value < 2) activeStep.value++;
    };

    // 环境自检
    const performCheck = async () => {
        checks.loading = true;
        invoke('check_admin_privileges').then(()=>{
            checks.admin = true;
            return invoke('check_camera_status');
        }).then(()=>{
            checks.camera = true;
            ElMessage.success('环境检查通过');
        }).catch((e)=>{
            ElMessage.error(formatObjectString(e));
        }).finally(()=>{
            checks.loading = false;
        });
    };

    onMounted(() => {
        performCheck();
    });

    // 部署
    const startDeployment = async () => {
        if (!checks.admin) {
            return ElMessage.error('权限不足，无法部署');
        }

        isDeploying.value = true;
        invoke('deploy_core_components').then(()=>{
            // 模拟进度条
            let progress = 0;
            const timer = setInterval(() => {
                progress += 25;
                deployProgress.value = progress;
                if (progress >= 100) {
                    clearInterval(timer);
                    isDeploying.value = false;
                    deployStatus.value = 'success';
                    ElMessage.success('DLL 与注册表配置完成');
                }
            }, 400);
        }).catch((error)=>{
            isDeploying.value = false;
            deployStatus.value = 'exception';
            ElMessageBox.alert(error, '部署失败', { type: 'error' });
        });
    };

    // 完成初始化，存入数据库
    const finishInit = () => {
        if (!authForm.username || !authForm.password) {
            return ElMessage.warning('请填写完整的账号密码信息');
        }

        ElMessageBox.alert('电脑将进入锁屏界面，5秒后自动解锁。<br>请不要手动解锁!!<br>如果5 秒内未解锁，代表测试失败，请手动解锁。', '通知', {
            confirmButtonText: '确定',
            dangerouslyUseHTMLString: true,
            callback: (action) => {
                if (action === 'confirm') {
                    handleLocalAccount(authForm, true)
                    isFinalizing.value = true;
                    invoke('test_win_logon', { userName: authForm.username, password: authForm.password }).then(result => {
                        optionsStore.saveOptions({is_initialized: 'true'}).then(errorList => {
                            if (errorList.length > 0) {
                                ElMessageBox.alert(formatObjectString(errorList), '保存设置失败', {
                                    confirmButtonText: '确定'
                                });
                            } else {
                                ElMessage.success('初始化成功');
                                router.push('/');
                            }
                        })
                    }).catch((error)=>{
                        ElMessageBox.alert(formatObjectString(error), '测试失败', {
                            confirmButtonText: '确定'
                        });
                    }).finally(()=>{
                        handleLocalAccount(authForm, false)
                        isFinalizing.value = false;
                    })
                }
            }
        });
    }
</script>

<template>
    <div class="init-container">
        <el-card class="init-card">
            <template #header>
                <div class="card-header">
                    <span>系统初始化向导</span>
                    <el-tag :type="initialized ? 'success' : 'warning'">
                        {{ initialized ? '已激活' : '待配置' }}
                    </el-tag>
                </div>
            </template>

            <el-steps :active="activeStep" finish-status="success" align-center>
                <el-step title="环境检测" />
                <el-step title="系统部署" />
                <el-step title="账户验证" />
            </el-steps>

            <div class="step-content">
                <div v-if="activeStep === 0">
                    <el-result icon="info" title="准备环境" sub-title="我们将检查摄像头权限及系统权限">
                        <template #extra>
                            <ul class="check-list">
                                <li>摄像头状态：
                                    <el-icon :color="checks.camera ? '#67C23A' : '#F56C6C'">
                                        <CircleCheckFilled v-if="checks.camera" />
                                        <CircleCloseFilled v-else />
                                    </el-icon>
                                </li>
                                <li>系统管理员权限：
                                    <el-icon color="#67C23A">
                                        <el-icon :color="checks.admin ? '#67C23A' : '#F56C6C'">
                                            <CircleCheckFilled v-if="checks.admin" />
                                            <CircleCloseFilled v-else />
                                        </el-icon>
                                    </el-icon>
                                </li>
                            </ul>
                            <el-button type="primary" @click="handleNextStep" style="display: block;" :loading="!(checks.camera && checks.admin)">继续部署</el-button>
                        </template>
                    </el-result>
                </div>

                <div v-if="activeStep === 1">
                    <div class="deploy-box">
                        <h3>正在部署 WinLogon 核心组件</h3>
                        <p>这包括复制 DLL 到 System32 并修改注册表以启用面容识别支持</p>
                        <div class="progress-wrapper">
                            <el-progress :percentage="deployProgress" :status="deployStatus" />
                        </div>
                        <el-button type="danger" :loading="isDeploying" @click="startDeployment"
                            v-if="deployProgress === 0">执行部署</el-button>
                        <!-- <el-button type="primary" :disabled="deployProgress < 100" @click="handleNextStep">下一步</el-button> -->
                        <el-button type="primary"  @click="handleNextStep">下一步</el-button>
                    </div>
                </div>

                <div v-if="activeStep === 2">
                    <div style="max-width: 450px; margin: 0 auto;">
                        <AccountAuthForm 
                            v-model="authForm" 
                            custom-tips="此密码仅用于 DLL 调起 WinLogon 认证，程序不会存储此密码"
                        />
                        <el-button type="success" style="width: 100%" @click="finishInit" :loading="isFinalizing">执行最终测试</el-button>
                    </div>
                </div>
            </div>
        </el-card>
    </div>
</template>

<style scoped>
    .init-container {
        display: flex;
        justify-content: center;
        align-items: center;
        height: 100%;
    }

    .init-card {
        width: 100%;
        max-width: 800px;
        height: 100%;
    }

    .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        font-weight: bold;
    }

    .step-content {
        margin-top: 40px;
        min-height: 300px;
    }

    .check-list {
        list-style: none;
        padding: 0;
        text-align: left;
        display: inline-block;
        margin-bottom: 20px;
    }

    .check-list li {
        margin: 10px 0;
        font-size: 15px;
        display: flex;
        align-items: center;
        gap: 10px;
    }

    .deploy-box {
        text-align: center;
        padding: 20px;
    }

    .progress-wrapper {
        margin: 30px 0;
    }
</style>